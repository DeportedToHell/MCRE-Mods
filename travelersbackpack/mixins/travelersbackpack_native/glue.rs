// Traveler's Backpack in the game, on the server: a backpack opens its menu
// from the main hand (`TravelersBackpackItem.use`, a right click; with sneak
// held it is placed instead, `place`) and placed (`TravelersBackpackBlock
// .useWithoutItem`). The server's menu holds what the backpack holds; when it
// closes, that goes back into the backpack: the item's `minecraft:container`,
// or the placed backpack's block entity. A placed backpack keeps the item's
// contents and tier, and gives them back to the item it drops.
//
// Uses are heard while the player's state is swapped in, the hub out of the
// game's hands: a menu opens at the next server tick, and what a closed menu
// held is written there too.
// Ported from Traveler's Backpack by Tiviacz1337 (LGPL-3.0).
use super::Game;
use super::travelersbackpack as tb;
use super::travelersbackpack_config::config as cfg;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::{NativeLootDrops, NativeScreen};
use horizon_client::text::Text;
use horizon_world::nbt::{Compound, Tag};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// Where an open backpack is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    /// The player's inventory slot.
    Hand(usize),
    /// Placed, in a dimension.
    Block([i32; 3]),
    /// Worn on the player's back.
    Worn,
    /// Worn on another player's back, opened by an operator (`/tb access`).
    Other(u128),
}

/// What a player asked of their backpacks, done at the next server tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    /// The equip button of a backpack opened from the inventory.
    Equip,
    /// The unequip button of the worn backpack.
    Unequip,
    /// The key (`KeybindHandler.OPEN_BACKPACK`): the worn backpack, else the
    /// first one in the inventory.
    Open,
    /// Sneaking, a right click on a placed backpack: worn when none is
    /// (quick equip), else taken into the empty hand (quick pick-up).
    TakePlaced([i32; 3]),
    /// Sneaking, shears on a placed backpack of a design: it drops as the
    /// standard one, holding what it held.
    Shear([i32; 3]),
    /// A placed backpack's model shows its tanks as its upgrades say.
    BlockTanks([i32; 3]),
    /// The sleeping bag button of the open menu.
    SleepingBag,
    /// A right click on that button (`toggleSleepingBag` with shift): the
    /// bag comes off the backpack, into the inventory.
    DetachBag,
    /// Sneaking, a sleeping bag used on a placed backpack: the backpack's bag
    /// is that one from now on, the old one dropped.
    ChangeBag([i32; 3]),
    /// An upgrade slot's tab opened or closed (its toggle).
    ToggleTab(usize),
    /// The jukebox tab's play or stop button.
    Jukebox(usize),
    /// The player died (`DeathHandler`, `backpackDeathPlace`): the worn
    /// backpack is placed where they lie, else dropped.
    Died,
    /// A sort button (`SortingButtons`): which, and whether shift was held.
    Sort(u8, bool),
    /// The tool slots shown or hidden (`ToolSlotsWidget`).
    ToggleTools,
    /// The more buttons shown or hidden (`MoreButton`).
    ToggleMore,
    /// The settings screen opened, or left (`SettingsWidget`).
    Settings(bool),
    /// A settings tab opened or closed: 1 the unsortables, 2 the memory.
    SettingsTab(u8),
    /// The unsortables tab's select all, or remove all.
    SelectAll(bool),
    /// The memory tab's match components button.
    ToggleMatch,
    /// The worn backpack shown on the back or not (`VisibilityWidget`).
    ToggleVisible,
    /// `ABILITY_SLIDER`: the worn or placed backpack's ability on or off.
    AbilitySlider,
    /// The void upgrade's trash bin on, or off and its choice voided.
    TrashBin,
    /// A tank of the open menu clicked.
    TankClick(usize),
    /// An open tab's remove button: its upgrade into the inventory.
    RemoveUpgrade(usize),
    /// A filter tab's button `k` of upgrade slot `i`.
    FilterButton(usize, usize),
    /// An upgrade switched on or off.
    ToggleEnabled(usize),
    /// The tag selector of upgrade slot `i` clicked on tag `k` of its list.
    FilterTag(usize, usize),
    /// Sneaking, the blank upgrade on a placed backpack: it comes apart.
    Unpack([i32; 3]),
    /// `allowOnlyEquippedBackpack`: the backpack in the hand put on.
    EquipHeld,
}

#[derive(Default)]
struct State {
    /// Operators who opened a placed backpack with `/tb access` (no reach limit).
    access: HashSet<u128>,
    /// Placed backpacks' menus closed while their dimension wasn't ticking:
    /// the backpack, its dimension and what to write into it.
    closed_elsewhere: Vec<([i32; 3], String, Vec<Option<ItemStack>>)>,
    /// Backpacks used, whose menus open at the next tick: the player, the
    /// backpack and the dimension it was used in.
    opening: Vec<(u128, Source, String)>,
    /// Each player's open backpack, its dimension and the menu's window.
    open: HashMap<u128, (Source, String, i32)>,
    /// What closed menus held, to go back into their backpacks.
    closed: Vec<(u128, i32, Vec<Option<ItemStack>>)>,
    /// Placed backpacks being broken: the backpack, and what its item gets
    /// (contents, tier, bag colour).
    broken: HashMap<[i32; 3], (String, Vec<Option<ItemStack>>, tb::Tier, i32, (Settings, Option<i32>))>,
    /// What players asked, with the dimension they were in.
    actions: Vec<(u128, Action, String)>,
    /// An equip or unequip waiting for its menu to close (its contents
    /// written back first).
    after_close: HashMap<u128, Action>,
    /// Players who joined, to be told who wears what.
    joined: Vec<u128>,
    /// Each open menu's upgrade and tool slots, which are the mod's (the
    /// storage is the menu's input): written back with the storage.
    extras: HashMap<u128, (tb::Tier, Vec<Option<ItemStack>>)>,
    /// When each wearer (or a player at a placed backpack) is fed next.
    feed_at: HashMap<u128, u64>,
    /// Menus closed to open again (the tanks upgrade went in or out).
    reopen: HashSet<u128>,
    /// Where wearers with the pickup upgrade touched stacks, for it to take.
    touched: Vec<(u128, [f64; 3])>,
    /// What each open menu last wrote back to its backpack.
    synced: HashMap<u128, Vec<Option<ItemStack>>>,
    /// Placed backpacks whose laid-out bag was just broken: it went back to
    /// them, so it drops nothing.
    bags_back: HashSet<[i32; 3]>,
    /// The game time each player last used the hose at.
    hose_tick: HashMap<u128, i64>,
    /// Players on a backpack's settings screen, and its state.
    settings: HashMap<u128, SettingsUi>,
    /// Players whose menu opens again as the settings screen.
    settings_enter: HashSet<u128>,
    /// Players whose menu opens again once an upgrade changed its slots.
    reopen_after: HashSet<u128>,
    /// The bags worn backpacks put down to sleep in: foot, head, dimension.
    temporary_bags: HashMap<u128, ([i32; 3], [i32; 3], String)>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// The slots of a backpack item: `size` of them, from its `minecraft:container`.
fn contents(stack: &ItemStack, size: usize) -> Vec<Option<ItemStack>> {
    let mut slots = vec![None; size];
    for (slot, item) in stack.data().map_or(&[][..], |d| &d.container[..]) {
        if let Some(s) = slots.get_mut(usize::from(*slot))
            && item.count > 0
        {
            *s = Some(item.clone());
        }
    }
    slots
}

/// The backpack item `stack` holding `slots` from now on.
fn set_contents(stack: &mut ItemStack, slots: Vec<Option<ItemStack>>) {
    // The sizes its slots were laid out by, kept from now on.
    if !has_sizes(stack) {
        let tier = stack_tier(stack);
        stamp_sizes(stack, tier);
    }
    let data = stack.data_mut();
    data.container = slots
        .into_iter()
        .enumerate()
        .filter_map(|(i, s)| Some((u8::try_from(i).ok()?, s.filter(|s| s.count > 0)?)))
        .collect();
    data.wire = Default::default();
    stack.tidy();
    refresh_look(stack);
}

/// What a backpack item's model shows (`RENDER_INFO`, `SLEEPING_BAG_COLOR`):
/// its custom model data, a flag for its tanks and its bag's colour (`none`).
fn refresh_look(stack: &mut ItemStack) {
    if !tb::is_backpack(&stack.item) {
        return;
    }
    let tier = stack_tier(stack);
    let upgrades = tier.storage..tier.storage + tier.upgrades;
    let tanks = stack.data().is_some_and(|d| {
        d.container.iter().any(|(slot, item)| upgrades.contains(&usize::from(*slot)) && &*item.item == TANKS_UPGRADE)
    });
    let mut look = Compound::new();
    look.insert("flags".into(), Tag::List(vec![Tag::Byte(i8::from(tanks))]));
    let mut strings = vec![Tag::String(bag_name(item_bag(stack)).into())];
    // `BackpackDynamicModel.addFluids`: with tanks, each tank's fluid in it
    // (its sprite, then its fill and colour), as the placed model draws it.
    if tanks && let Some(levels) = tank_levels(&contents(stack, tier.slots()), tier) {
        let mut fills = Vec::new();
        let mut colours = Vec::new();
        for (code, fill) in levels {
            let (sprite, colour) = match code {
                c if c <= 0 => ("", 0xFFFFFF),
                1 => ("water", 0x3F76E4),
                2 => ("lava", 0xFFFFFF),
                3 => ("milk", 0xFFFFFF),
                c => ("water", tank_code_colour(c) as i32),
            };
            strings.push(Tag::String(sprite.into()));
            fills.push(Tag::Float(if sprite.is_empty() { 0.0f32 } else { fill }.to_bits()));
            colours.push(Tag::Int(colour));
        }
        look.insert("floats".into(), Tag::List(fills));
        look.insert("colors".into(), Tag::List(colours));
    }
    look.insert("strings".into(), Tag::List(strings));
    let data = stack.data_mut();
    data.set_other("minecraft:custom_model_data", Some(Tag::Compound(look)));
    data.wire = Default::default();
    stack.tidy();
}

/// A backpack item's tier ordinal (`ModDataComponents.TIER`, 0 when unset).
fn item_tier(game: &Game<'_>, stack: &ItemStack) -> i32 {
    match game.native_component(stack, tb::TIER_COMPONENT) {
        Ok(Some(Tag::Int(n))) => *n,
        _ => 0,
    }
}

/// A placed backpack's tier ordinal, kept in its block entity's compound.
fn block_tier(nbt: &Compound) -> i32 {
    match nbt.get(&horizon_world::nbt::JavaString::from("tier")) {
        Some(Tag::Int(n)) => *n,
        _ => 0,
    }
}

/// Tier `n` as the config makes a new backpack of it (`Tiers.getStorageSlots` …).
fn configured_tier(n: i32) -> tb::Tier {
    let [storage, upgrades, tools] = usize::try_from(n).ok().and_then(|n| cfg().tier_sizes.get(n).copied()).unwrap_or([27, 2, 2]);
    tb::tier(n).sized(storage, upgrades, tools)
}

/// `BackpackWrapper.initializeSize`: a backpack keeps the sizes it was made
/// with; one without them holding nothing takes its tier's from the config,
/// one holding things (from before the sizes were kept) its tier's first.
fn sized_tier(n: i32, sizes: [Option<i32>; 3], empty: bool) -> tb::Tier {
    match sizes {
        [Some(s), Some(u), Some(t)] => {
            let size = |v: i32| usize::try_from(v).unwrap_or(0);
            tb::tier(n).sized(size(s), size(u), size(t))
        }
        _ if empty => configured_tier(n),
        _ => tb::tier(n),
    }
}

fn has_sizes(stack: &ItemStack) -> bool {
    stack.data().is_some_and(|d| d.other(tb::STORAGE_COMPONENT).is_some())
}

/// A backpack item's tier with its own sizes.
fn stack_tier(stack: &ItemStack) -> tb::Tier {
    let int = |id: &str| match stack.data().and_then(|d| d.other(id)) {
        Some(Tag::Int(n)) => Some(*n),
        _ => None,
    };
    let empty = stack.data().is_none_or(|d| d.container.is_empty());
    sized_tier(
        int(tb::TIER_COMPONENT).unwrap_or(0),
        [int(tb::STORAGE_COMPONENT), int(tb::UPGRADES_COMPONENT), int(tb::TOOLS_COMPONENT)],
        empty,
    )
}

/// The backpack item `stack` of `tier`, with its sizes (`upgradeInventory`).
fn stamp_sizes(stack: &mut ItemStack, tier: tb::Tier) {
    let data = stack.data_mut();
    for (id, n) in [(tb::STORAGE_COMPONENT, tier.storage), (tb::UPGRADES_COMPONENT, tier.upgrades), (tb::TOOLS_COMPONENT, tier.tools)] {
        data.set_other(&format!("!{id}"), None);
        data.set_other(id, Some(Tag::Int(n as i32)));
    }
    data.wire = Default::default();
    stack.tidy();
}

/// The backpack item made of a placed one of `tier`: its tier and sizes.
fn set_item_tier(game: &mut Game<'_>, stack: &mut ItemStack, tier: tb::Tier) {
    if tier.index() != 0 {
        let _ = game.native_set_component(stack, tb::TIER_COMPONENT, Tag::Int(tier.index()));
    }
    stamp_sizes(stack, tier);
    refresh_look(stack);
}

/// A placed backpack's tier with its own sizes, kept beside its tier.
fn placed_tier(nbt: &Compound, items: &[Option<ItemStack>]) -> tb::Tier {
    let int = |k: &str| match nbt.get(&horizon_world::nbt::JavaString::from(k)) {
        Some(Tag::Int(n)) => Some(*n),
        _ => None,
    };
    sized_tier(block_tier(nbt), [int("storage_slots"), int("upgrade_slots"), int("tool_slots")], items.iter().all(Option::is_none))
}

fn set_block_sizes(nbt: &mut Compound, tier: tb::Tier) {
    nbt.insert("storage_slots".into(), Tag::Int(tier.storage as i32));
    nbt.insert("upgrade_slots".into(), Tag::Int(tier.upgrades as i32));
    nbt.insert("tool_slots".into(), Tag::Int(tier.tools as i32));
}

// ---- the worn backpack (`BackpackAttachment`) ---------------------------------------------

/// The player's persistent data key holding the backpack on their back.
const WORN: &str = "travelersbackpack:worn";
/// Mod messages: the client's key asks the server to open (empty), the server
/// tells clients who wears which design (`<name>\n<backpack id>`, the id empty
/// for none: `ClientboundSyncAttachmentPacket`).
const OPEN_CHANNEL: &str = "travelersbackpack:open";
/// `OPEN_BACKPACK` with a slot: the backpack in that inventory slot (`allowOpeningFromSlot`).
const OPEN_SLOT_CHANNEL: &str = "travelersbackpack:open_slot";
const WORN_CHANNEL: &str = "travelersbackpack:worn";
/// The server tells a wearer which ability works for them now (empty: none).
const ABILITY_CHANNEL: &str = "travelersbackpack:ability";
/// The client's keys: sort the open backpack (shift: its way), the worn
/// one's ability, an upgrade slot of the worn one on or off.
const SORT_CHANNEL: &str = "travelersbackpack:sort";
const SORT_SHIFT_CHANNEL: &str = "travelersbackpack:sort_shift";
const ABILITY_TOGGLE_CHANNEL: &str = "travelersbackpack:ability_toggle";
const TOGGLE_UPGRADE_CHANNELS: [&str; 4] = [
    "travelersbackpack:toggle_upgrade_0",
    "travelersbackpack:toggle_upgrade_1",
    "travelersbackpack:toggle_upgrade_2",
    "travelersbackpack:toggle_upgrade_3",
];


/// `UPGRADE_TAB` with `UPGRADE_ENABLED`: upgrade slot `i` of the worn
/// backpack on or off, if it can be; not while its menu is open.
fn toggle_worn_upgrade(game: &mut Game<'_>, uuid: u128, i: usize) {
    if worn_menu_open(uuid) {
        return;
    }
    let Some(mut stack) = worn(game, uuid) else {
        return;
    };
    let tier = stack_tier(&stack);
    if i >= tier.upgrades {
        return;
    }
    let mut items = contents(&stack, tier.slots());
    let Some(upgrade) = items[tier.storage + i].as_mut().filter(|u| can_switch_off(&u.item)) else {
        return;
    };
    let on = upgrade_enabled(upgrade);
    set_upgrade_enabled(upgrade, !on);
    set_contents(&mut stack, items);
    set_worn(game, uuid, Some(&stack));
    tell_worn(game, uuid);
}
/// The native mod's id, for its key binding.
const MOD_ID: &str = "travelersbackpack_native";
/// The port's version (`horizon-native.json`).
const VERSION: &str = "0.9.8";

/// The backpack `uuid` wears, if any.
fn worn(game: &Game<'_>, uuid: u128) -> Option<ItemStack> {
    let Some(Tag::Compound(data)) = game.native_player_persistent_data(uuid) else {
        return None;
    };
    data.get(&horizon_world::nbt::JavaString::from(WORN))
        .and_then(horizon_items::item_nbt::item_from_nbt)
        .filter(|s| tb::is_backpack(&s.item))
}

/// `uuid` wears `stack` from now on (None: nothing). False when the player
/// isn't reached.
fn set_worn(game: &mut Game<'_>, uuid: u128, stack: Option<&ItemStack>) -> bool {
    let mut data = match game.native_player_persistent_data(uuid) {
        Some(Tag::Compound(data)) => data,
        Some(_) => Compound::new(),
        None => return false,
    };
    match stack {
        Some(stack) => {
            data.insert(WORN.into(), horizon_items::item_nbt::item_to_nbt(stack));
        }
        None => {
            data.remove(&horizon_world::nbt::JavaString::from(WORN));
        }
    }
    game.native_set_player_persistent_data(uuid, Tag::Compound(data))
}

/// Tells every player what `uuid` wears now (`AttachmentUtils.synchronise`),
/// for its model on their back.
fn tell_worn(game: &mut Game<'_>, uuid: u128) {
    let Some(host) = game.lan.as_ref() else {
        return;
    };
    let Some(name) = host.hub.guests.iter().find(|g| g.uuid == uuid).map(|g| g.name.clone()) else {
        return;
    };
    let players: Vec<u128> = host.hub.guests.iter().filter(|g| g.placed).map(|g| g.uuid).collect();
    // The design, and whether its tanks show.
    // Hidden in its settings (`IS_VISIBLE`): nothing on the back.
    let design = worn(game, uuid)
        .filter(|s| !item_settings(s).hidden)
        .map(|s| {
            let tier = stack_tier(&s);
            let items = contents(&s, tier.slots());
            let tanks = shows_tanks(&items, tier);
            // And the tanks' fluids, for the model.
            let fluids = tank_tag(&items, tier).map(|t| t.iter().map(i32::to_string).collect::<Vec<_>>().join(",")).unwrap_or_default();
            let dye = item_dye(&s).map(|c| c.to_string()).unwrap_or_default();
            format!("{}\t{tanks}\t{fluids}\t{}\t{dye}", s.item, bag_name(item_bag(&s)))
        })
        .unwrap_or_default();
    // Its lantern on (`RenderInfo.hasLantern`), shown or not: its light.
    let lantern = worn(game, uuid).is_some_and(|s| {
        let tier = stack_tier(&s);
        upgrades_in(&contents(&s, tier.slots()), tier).iter().any(|u| u == "travelersbackpack:lantern_upgrade")
    });
    let tools = worn(game, uuid)
        .filter(|s| !item_settings(s).hidden)
        .map(|s| {
            let tier = stack_tier(&s);
            let shown: Vec<ItemStack> = contents(&s, tier.slots())[tier.storage + tier.upgrades..].iter().flatten().cloned().collect();
            let snbt = |t: &ItemStack| horizon_world::snbt_text::to_snbt(&horizon_items::item_nbt::item_to_nbt(t));
            match shown.as_slice() {
                [] => String::new(),
                [one] => snbt(one),
                [first, .., last] => format!("{}\t{}", snbt(first), snbt(last)),
            }
        })
        .unwrap_or_default();
    let message = format!("{name}\n{design}\n{}\n{tools}", u8::from(lantern));
    for player in players {
        let _ = game.native_net_send_to_player(player, WORN_CHANNEL, message.as_bytes());
    }
}

/// The first free slot of a guest's inventory, the hotbar first
/// (`Inventory.getFreeSlot`).
fn free_slot(inventory: &[Option<ItemStack>]) -> Option<usize> {
    inventory.iter().take(36).position(Option::is_none)
}

/// `stack` into player `uuid`'s inventory, else dropped at them.
fn give_back(game: &mut Game<'_>, uuid: u128, stack: ItemStack) {
    let left = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)).and_then(|g| {
        match free_slot(&g.inventory) {
            Some(k) => {
                g.inventory[k] = Some(stack.clone());
                None
            }
            None => Some((stack.clone(), g.position)),
        }
    });
    if let Some((stack, at)) = left {
        game.spawn_item(stack, at, [0.0; 3], 40);
    }
}

/// `ServerActions.equipBackpack`: the backpack in inventory slot `slot` goes
/// on the back; one worn already comes into that slot (`swapBackpack`).
fn equip(game: &mut Game<'_>, uuid: u128, slot: usize) {
    let old = worn(game, uuid);
    let Some(host) = game.lan.as_mut() else {
        return;
    };
    let Some(g) = host.hub.guests.iter_mut().find(|g| g.placed && g.uuid == uuid) else {
        return;
    };
    let Some(stack) = g.inventory.get_mut(slot).and_then(Option::take) else {
        return;
    };
    if !tb::is_backpack(&stack.item) {
        g.inventory[slot] = Some(stack);
        return;
    }
    g.inventory[slot] = old;
    if set_worn(game, uuid, Some(&stack)) {
        tell_worn(game, uuid);
    } else if let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)) {
        // Not stored: the backpack goes back where it was.
        g.inventory[slot] = Some(stack);
    }
}

/// `ServerActions.unequipBackpack`: the worn backpack comes into the
/// inventory, when it has room.
fn unequip(game: &mut Game<'_>, uuid: u128) {
    let Some(stack) = worn(game, uuid) else {
        return;
    };
    let Some(host) = game.lan.as_mut() else {
        return;
    };
    let Some(g) = host.hub.guests.iter_mut().find(|g| g.placed && g.uuid == uuid) else {
        return;
    };
    let Some(slot) = free_slot(&g.inventory) else {
        return;
    };
    g.inventory[slot] = Some(stack);
    if set_worn(game, uuid, None) {
        tell_worn(game, uuid);
    } else if let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)) {
        g.inventory[slot] = None;
    }
}

/// The placed backpack at `pos` as the item it is, taken from the world.
fn take_placed(game: &mut Game<'_>, dimension: &str, pos: [i32; 3]) -> Option<ItemStack> {
    let block = game.native_block(pos);
    if game.dim.id() != dimension || !tb::is_backpack(&block) {
        return None;
    }
    flush_block_menus(game, pos);
    let mut taken = None;
    game.native_with_mod_block_entity(pos, |m| {
        let items: Vec<Option<ItemStack>> = m.items.iter_mut().map(Option::take).collect();
        let tier = placed_tier(&m.nbt, &items);
        taken = Some((items, tier, block_bag(&m.nbt), (block_settings(&m.nbt), block_dye(&m.nbt))));
    });
    let (mut items, tier, bag, (settings, dye)) = taken?;
    stop_jukeboxes(game, items.iter_mut().flatten());
    fold_bag_of(game, dimension, pos);
    unlight(game, dimension, pos);
    let air = game.native_default_state("minecraft:air")?;
    if !game.native_set_block(dimension, pos, air, 3) {
        return None;
    }
    let mut stack = ItemStack { item: block.as_str().into(), count: 1, damage: 0, extra: None };
    set_contents(&mut stack, items);
    set_item_bag(&mut stack, bag);
    set_item_settings(&mut stack, &settings);
    set_item_dye(&mut stack, dye);
    set_item_tier(game, &mut stack, tier);
    Some(stack)
}

/// `RightClickHandler`'s blank upgrade: what the placed backpack holds, its
/// tools and upgrades, the tier upgrades it took, and the bare backpack (no
/// bag if it had none) drop above it, and it is gone.
fn unpack(game: &mut Game<'_>, dimension: &str, pos: [i32; 3]) {
    if game.dim.id() != dimension || !tb::is_backpack(&game.native_block(pos)) {
        return;
    }
    flush_block_menus(game, pos);
    let block = game.native_block(pos);
    let mut taken = None;
    game.native_with_mod_block_entity(pos, |m| {
        let items: Vec<ItemStack> = m.items.iter_mut().filter_map(Option::take).filter(|s| s.count > 0).collect();
        taken = Some((items, block_tier(&m.nbt), block_bag(&m.nbt)));
    });
    let Some((mut drops, tier, bag)) = taken else {
        return;
    };
    stop_jukeboxes(game, drops.iter_mut());
    fold_bag_of(game, dimension, pos);
    unlight(game, dimension, pos);
    let Some(air) = game.native_default_state("minecraft:air") else {
        return;
    };
    if !game.native_set_block(dimension, pos, air, 3) {
        return;
    }
    for upgrade in ["iron", "gold", "diamond", "netherite"].iter().take(usize::try_from(tier).unwrap_or(0)) {
        drops.push(ItemStack::new(&format!("travelersbackpack:{upgrade}_tier_upgrade"), 1));
    }
    let mut backpack = ItemStack::new(&block, 1);
    if bag == -1 {
        set_item_bag(&mut backpack, -1);
    }
    drops.push(backpack);
    let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.5, f64::from(pos[2]) + 0.5];
    for stack in drops {
        game.spawn_item(stack, at, super::dispenser::new_item_motion(), 10);
    }
}

/// Blocks a dead player's backpack may take the place of.
fn replaceable(block: &str) -> bool {
    block.ends_with("air")
        || ["minecraft:short_grass", "minecraft:tall_grass", "minecraft:fern", "minecraft:snow"].contains(&block)
}

/// `BackpackDeathHelper.onPlayerDrops`: the worn backpack placed where its
/// wearer died (the first free block up from there, then around), holding
/// what it held; dropped when there's no room.
fn place_on_death(game: &mut Game<'_>, uuid: u128, dimension: &str, at: [i32; 3]) {
    let Some(stack) = worn(game, uuid) else {
        return;
    };
    if game.ui.rules.bool("keep_inventory") {
        return;
    }
    // `BackpackManager.addBackpack`: a backup, for `/tb restore`.
    save_backup(game, uuid, &stack);
    if !set_worn(game, uuid, None) {
        return;
    }
    tell_worn(game, uuid);
    // `voidProtection`: a death below the world places it 5 above its bottom.
    let mut at = at;
    let min_y = game.world.shape.min_y;
    if cfg().void_protection && at[1] <= min_y {
        at[1] = min_y + 5;
    }
    let placed = if !cfg().backpack_death_place {
        None
    } else if cfg().backpack_force_death_place {
        // `placeBackpack` forced: the first air up the column where the player died.
        let top = game.world.shape.max_y();
        let spot = (at[1]..top).map(|y| [at[0], y, at[2]]).find(|&p| game.native_block(p).ends_with("air"));
        game.native_default_state(&stack.item)
            .zip(spot)
            .and_then(|(state, spot)| game.native_set_block(dimension, spot, state, 3).then_some(spot))
    } else {
        let spots = (0..8).map(|dy| [at[0], at[1] + dy, at[2]]).chain((-1..=1).flat_map(move |dx| {
            (-1..=1).flat_map(move |dz| (0..3).map(move |dy| [at[0] + dx, at[1] + dy, at[2] + dz]))
        }));
        game.native_default_state(&stack.item).and_then(|state| {
            let spot = spots.into_iter().find(|&p| replaceable(&game.native_block(p)))?;
            game.native_set_block(dimension, spot, state, 3).then_some(spot)
        })
    };
    // `ClientboundSendMessagePacket`: where it went, for the player's chat.
    let (dropped, where_) = match placed {
        Some(spot) => (false, spot),
        None => (true, at),
    };
    let message = format!("{}\t{}\t{}\t{}", u8::from(dropped), where_[0], where_[1], where_[2]);
    let _ = game.native_net_send_to_player(uuid, MESSAGE_CHANNEL, message.as_bytes());
    match placed {
        Some(spot) => {
            let mut items = contents(&stack, tb::MAX_SLOTS);
            let tier = item_tier(game, &stack);
            let sized = stack_tier(&stack);
            let tanks = shows_tanks(&items, sized);
            game.native_with_mod_block_entity(spot, |m| {
                items.resize(m.items.len().max(tb::MAX_SLOTS), None);
                m.items = std::mem::take(&mut items);
                m.nbt.insert("tier".into(), Tag::Int(tier));
                set_block_sizes(&mut m.nbt, sized);
                m.nbt.insert(BAG_COLOUR_NBT.into(), Tag::Int(item_bag(&stack)));
                set_block_dye(&mut m.nbt, item_dye(&stack));
                set_block_settings(&mut m.nbt, &item_settings(&stack));
            });
            set_block_tanks(game, dimension, spot, tanks);
        }
        None => {
            let at = at.map(|c| f64::from(c) + 0.5);
            game.spawn_item(stack, at, super::dispenser::new_item_motion(), 40);
        }
    }
}

/// One player's action at the tick.
fn act(game: &mut Game<'_>, uuid: u128, action: Action, dimension: &str) {
    // A placed backpack is in its own dimension's tick.
    if matches!(
        action,
        Action::TakePlaced(_)
            | Action::Unpack(_)
            | Action::Shear(_)
            | Action::Died
            | Action::BlockTanks(_)
            | Action::SleepingBag
            | Action::DetachBag
            | Action::ChangeBag(_)
            | Action::Sort(..)
            | Action::ToggleTools
            | Action::ToggleMore
            | Action::Settings(_)
            | Action::SettingsTab(_)
            | Action::SelectAll(_)
            | Action::ToggleMatch
            | Action::ToggleVisible
            | Action::AbilitySlider
            | Action::TrashBin
            | Action::TankClick(_)
            | Action::RemoveUpgrade(_)
            | Action::FilterButton(..)
            | Action::ToggleEnabled(_)
            | Action::FilterTag(..)
    )
        && game.dim.id() != dimension
    {
        with(|s| s.actions.push((uuid, action, dimension.to_owned())));
        return;
    }
    let open = with(|s| s.open.get(&uuid).cloned());
    match action {
        Action::Jukebox(i) => {
            if let Some((source, dim, window)) = open {
                jukebox(game, uuid, source, &dim, window, i);
            }
        }
        Action::ToggleTab(i) => {
            if let Some((_, _, window)) = open {
                toggle_tab(game, uuid, window, i);
            }
        }
        Action::SleepingBag => {
            if !cfg().enable_sleeping_bag || (matches!(open, Some((Source::Worn, _, _))) && !cfg().quick_sleeping_bag) {
                return;
            }
            if let Some((source, _, window)) = open {
                use_sleeping_bag(game, uuid, dimension, source, window);
            }
        }
        Action::DetachBag => {
            if let Some((source, _, window)) = open {
                detach_bag(game, uuid, dimension, source, window);
            }
        }
        Action::ChangeBag(pos) => {
            if cfg().enable_sleeping_bag {
                flush_block_menus(game, pos);
                change_bag(game, uuid, dimension, pos);
            }
        }
        Action::Sort(kind, shift) => sort_button(game, uuid, kind, shift),
        Action::TrashBin => trash_bin(game, uuid),
        Action::TankClick(side) => tank_clicked(game, uuid, side),
        Action::FilterButton(i, k) => filter_button(game, uuid, i, k),
        Action::ToggleEnabled(i) => toggle_enabled(game, uuid, i),
        Action::FilterTag(i, k) => filter_tag_clicked(game, uuid, i, k),
        Action::RemoveUpgrade(i) => {
            if let Some((_, _, window)) = open {
                remove_upgrade(game, uuid, window, i);
            }
        }
        Action::ToggleTools
        | Action::ToggleMore
        | Action::Settings(_)
        | Action::SettingsTab(_)
        | Action::SelectAll(_)
        | Action::ToggleMatch
        | Action::ToggleVisible
        | Action::AbilitySlider => settings_button(game, uuid, action),
        Action::BlockTanks(pos) => {
            let tanks = game.native_mod_block_entity(pos).filter(|m| m.id == tb::BLOCK_ENTITY).map(|m| {
                let tier = placed_tier(&m.nbt, &m.items);
                shows_tanks(&m.items, tier)
            });
            if let Some(tanks) = tanks {
                set_block_tanks(game, dimension, pos, tanks);
            }
        }
        Action::Died => {
            // A menu still open: what it shows goes into its backpack first.
            if with(|s| s.open.contains_key(&uuid)) {
                sync_menu(game, uuid);
                close_now(game, uuid);
            }
            // The body is where it fell until the player respawns.
            let at = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.position);
            if let Some(at) = at {
                place_on_death(game, uuid, dimension, at.map(|c| c.floor() as i32));
            }
        }
        // The menu closes first, its contents written back; then the equip.
        Action::Equip | Action::Unequip => {
            let Some((source, _, window)) = open else {
                return;
            };
            let fits = matches!((action, source), (Action::Equip, Source::Hand(_)) | (Action::Unequip, Source::Worn));
            let Some(host) = game.lan.as_mut() else {
                return;
            };
            let Some(conn) = host.hub.guests.iter().find(|g| g.uuid == uuid).map(|g| g.conn) else {
                return;
            };
            if fits && horizon_client::lan::containers::travelersbackpack_menu::close(&mut host.hub, conn, window) {
                with(|s| s.after_close.insert(uuid, action));
            }
        }
        Action::Open => {
            if open.is_some() {
                return;
            }
            let source = if worn(game, uuid).is_some() {
                Some(Source::Worn)
            } else {
                // `KeybindHandler`: not wearing one, the first in the inventory.
                game.lan
                    .as_ref()
                    .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid))
                    .and_then(|g| {
                        g.inventory.iter().take(36).position(|s| s.as_ref().is_some_and(|s| tb::is_backpack(&s.item)))
                    })
                    .map(Source::Hand)
            };
            if let Some(source) = source {
                with(|s| s.opening.push((uuid, source, dimension.to_owned())));
            }
        }
        Action::Shear(pos) => shear(game, uuid, dimension, pos),
        Action::Unpack(pos) => unpack(game, dimension, pos),
        Action::EquipHeld => {
            if worn(game, uuid).is_some() {
                return;
            }
            let held = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)).and_then(|g| {
                let slot = g.selected;
                let stack = g.inventory.get_mut(slot)?.take()?;
                if tb::is_backpack(&stack.item) {
                    Some(stack)
                } else {
                    g.inventory[slot] = Some(stack);
                    None
                }
            });
            if let Some(stack) = held {
                if set_worn(game, uuid, Some(&stack)) {
                    tell_worn(game, uuid);
                } else if let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)) {
                    let slot = g.selected;
                    g.inventory[slot] = Some(stack);
                }
            }
        }
        Action::TakePlaced(pos) => {
            let wearing = worn(game, uuid).is_some();
            let selected =
                game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.selected);
            let Some(selected) = selected else {
                return;
            };
            let Some(stack) = take_placed(game, dimension, pos) else {
                return;
            };
            if !wearing && set_worn(game, uuid, Some(&stack)) {
                tell_worn(game, uuid);
                return;
            }
            // Quick pick-up, into the (empty) hand; else wherever there's room,
            // and the ground when there's none.
            let slot = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)).and_then(|g| {
                let slot =
                    if g.inventory.get(selected).is_some_and(Option::is_none) { Some(selected) } else { free_slot(&g.inventory) }?;
                g.inventory[slot] = Some(stack.clone());
                Some(slot)
            });
            if slot.is_none() {
                let at = pos.map(|c| f64::from(c) + 0.5);
                game.spawn_item(stack, at, super::dispenser::new_item_motion(), 10);
            }
        }
    }
}

// ---- registration -------------------------------------------------------------------------

/// The tier's component (`ModDataComponents.TIER`).
pub(super) fn component_types(types: &mut Vec<String>) {
    types.push(tb::TIER_COMPONENT.into());
    types.push(tb::STORAGE_COMPONENT.into());
    types.push(tb::UPGRADES_COMPONENT.into());
    types.push(tb::TOOLS_COMPONENT.into());
}

/// A placed backpack has a block entity holding the item's slots.
pub(super) fn block_entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if entity.is_none() && tb::is_backpack(block) {
        *entity = Some((tb::BLOCK_ENTITY.into(), Compound::new()));
    }
}

/// Its slots: as many as the biggest tier's, none reached from the sides yet.
pub(super) fn block_entity_container(block: &str, container: &mut Option<horizon_client::containers::ModContainer>) {
    if container.is_none() && tb::is_backpack(block) {
        *container = Some(horizon_client::containers::ModContainer {
            slots: tb::MAX_SLOTS,
            faces: Default::default(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            menu: None,
        });
    }
}

// ---- using a backpack ---------------------------------------------------------------------

/// `TravelersBackpackItem.use`: the backpack in the hand opens, or with
/// `allowOnlyEquippedBackpack` is put on when none is worn.
fn hand_use(game: &mut Game<'_>) {
    if !cfg().allow_only_equipped_backpack {
        open_later(game, Source::Hand(usize::MAX));
        return;
    }
    let uuid = game.native_acting_id();
    let dimension = game.dim.id().to_owned();
    with(|s| s.actions.push((uuid, Action::EquipHeld, dimension)));
}

fn open_later(game: &mut Game<'_>, source: Source) {
    let uuid = game.native_acting_id();
    let dimension = game.dim.id().to_owned();
    // One click can be heard twice (each hand): one menu per player per tick.
    with(|s| {
        if !s.opening.iter().any(|(u, _, _)| *u == uuid) {
            s.opening.push((uuid, source, dimension));
        }
    });
}

/// `TravelersBackpackItem.use`: in the air, a backpack in the main hand opens
/// (not while sneaking).
pub(super) fn use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if !*claimed && &*stack.item == HOSE {
        hose_use(game, claimed);
        return;
    }
    if *claimed || !tb::is_backpack(&stack.item) || game.native_sneaking() {
        return;
    }
    *claimed = true;
    hand_use(game);
}

/// `TravelersBackpackItem.useOn`: on a block, sneaking places it, else it opens.
pub(super) fn use_item_on(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3], claimed: &mut bool) {
    if !*claimed && &*stack.item == HOSE {
        hose_use(game, claimed);
        return;
    }
    // A dyed backpack on a cauldron of water is washed there (`CauldronInteraction`).
    if item_dye(stack).is_some() && game.native_block(pos) == "minecraft:water_cauldron" {
        return;
    }
    if *claimed || !tb::is_backpack(&stack.item) || game.native_sneaking() {
        return;
    }
    *claimed = true;
    hand_use(game);
}

/// `TravelersBackpackBlock.useWithoutItem`: a placed backpack opens. Sneaking
/// (`RightClickHandler`), it is worn when none is (quick equip), else taken
/// into an empty hand (quick pick-up); a block in the hand is placed instead.
pub(super) fn right_click_block(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if !*canceled {
        washed(game, pos);
    }
    if *canceled || !tb::is_backpack(&game.native_block(pos)) {
        return;
    }
    if !game.native_sneaking() {
        *canceled = true;
        open_later(game, Source::Block(pos));
        return;
    }
    let uuid = game.native_acting_id();
    let held = game.inventory.get(game.selected).cloned().flatten();
    if held.as_ref().is_some_and(|s| &*s.item == "minecraft:shears") {
        if game.native_block(pos) != STANDARD {
            *canceled = true;
            let dimension = game.dim.id().to_owned();
            with(|s| s.actions.push((uuid, Action::Shear(pos), dimension)));
        }
        return;
    }
    if held.as_ref().is_some_and(|s| bag_colour_of(&s.item).is_some()) {
        *canceled = true;
        let dimension = game.dim.id().to_owned();
        with(|s| s.actions.push((uuid, Action::ChangeBag(pos), dimension)));
        return;
    }
    // `BLANK_UPGRADE`: the backpack taken apart.
    if held.as_ref().is_some_and(|s| &*s.item == "travelersbackpack:blank_upgrade") {
        *canceled = true;
        let dimension = game.dim.id().to_owned();
        with(|s| s.actions.push((uuid, Action::Unpack(pos), dimension)));
        return;
    }
    let block_in_hand = held.as_ref().is_some_and(|s| game.native_default_state(&s.item).is_some());
    // Quick equip (`rightClickEquip`), else quick pick-up into an empty hand.
    let take = if worn(game, uuid).is_none() && cfg().right_click_equip { !block_in_hand } else { held.is_none() };
    if take {
        *canceled = true;
        let dimension = game.dim.id().to_owned();
        with(|s| {
            if !s.actions.iter().any(|(u, _, _)| *u == uuid) {
                s.actions.push((uuid, Action::TakePlaced(pos), dimension));
            }
        });
    }
}

/// `TravelersBackpackItem.place`: the placed backpack keeps the item's
/// contents and tier (`BackpackBlockEntity.setBackpack`).
pub(super) fn place_block(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3]) {
    if !tb::is_backpack(&stack.item) {
        return;
    }
    let mut items = contents(stack, tb::MAX_SLOTS);
    let tier = item_tier(game, stack);
    let sized = stack_tier(stack);
    let bag = item_bag(stack);
    let dye = item_dye(stack);
    let settings = item_settings(stack);
    game.native_with_mod_block_entity(pos, |m| {
        set_block_dye(&mut m.nbt, dye);
        set_block_settings(&mut m.nbt, &settings);
        items.resize(m.items.len().max(tb::MAX_SLOTS), None);
        m.items = std::mem::take(&mut items);
        m.nbt.insert("tier".into(), Tag::Int(tier));
        set_block_sizes(&mut m.nbt, sized);
        m.nbt.insert(BAG_COLOUR_NBT.into(), Tag::Int(bag));
    });
    let (uuid, dimension) = (game.native_acting_id(), game.dim.id().to_owned());
    with(|s| s.actions.push((uuid, Action::BlockTanks(pos), dimension)));
}

/// A placed backpack being broken: what it holds goes to the item it drops.
/// Its block entity is emptied, so its contents don't spill like a chest's.
pub(super) fn destroy_block(game: &mut Game<'_>, pos: [i32; 3]) {
    let block = game.native_block(pos);
    if is_sleeping_bag(&block) {
        bag_broken(game, pos);
        return;
    }
    if !tb::is_backpack(&block) {
        return;
    }
    melon_broken(game, pos);
    flush_block_menus(game, pos);
    let dimension = game.dim.id().to_owned();
    fold_bag_of(game, &dimension, pos);
    unlight(game, &dimension, pos);
    let mut taken = None;
    game.native_with_mod_block_entity(pos, |m| {
        let items: Vec<Option<ItemStack>> = m.items.iter_mut().map(Option::take).collect();
        let tier = placed_tier(&m.nbt, &items);
        taken = Some((items, tier, block_bag(&m.nbt), (block_settings(&m.nbt), block_dye(&m.nbt))));
    });
    let (mut items, tier, bag, (settings, dye)) = taken.unwrap_or((Vec::new(), tb::tier(0), RED, (Settings::default(), None)));
    stop_jukeboxes(game, items.iter_mut().flatten());
    if game.creative {
        // `playerWillDestroy`: in creative the backpack still drops, as an item
        // holding what it held (no loot is rolled there).
        let mut stack = ItemStack { item: block.as_str().into(), count: 1, damage: 0, extra: None };
        set_contents(&mut stack, items);
        set_item_bag(&mut stack, bag);
        set_item_settings(&mut stack, &settings);
        set_item_dye(&mut stack, dye);
        set_item_tier(game, &mut stack, tier);
        let at = pos.map(|c| f64::from(c) + 0.5);
        game.spawn_item(stack, at, super::dispenser::new_item_motion(), 10);
        return;
    }
    with(|s| s.broken.insert(pos, (block, items, tier, bag, (settings, dye))));
}

/// The backpack a broken one drops, holding its contents and tier
/// (`BackpackBlockEntity`'s components, copied by its loot table).
pub(super) fn loot_drops(game: &mut Game<'_>, context: &NativeLootDrops<'_>, drops: &mut Vec<ItemStack>) {
    mob_backpack_drop(game, context, drops);
    let Some((pos, state)) = context.block else {
        return;
    };
    sleeping_bag_drops(game, pos, state, drops);
    hay_drops(game, context, drops);
    let Some((block, items, tier, bag, (settings, dye))) = with(|s| s.broken.remove(&pos)) else {
        return;
    };
    let i = match drops.iter().position(|d| tb::is_backpack(&d.item)) {
        Some(i) => i,
        None => {
            drops.push(ItemStack { item: block.as_str().into(), count: 1, damage: 0, extra: None });
            drops.len() - 1
        }
    };
    let stack = &mut drops[i];
    set_contents(stack, items);
    set_item_bag(stack, bag);
    set_item_settings(stack, &settings);
    set_item_dye(stack, dye);
    set_item_tier(game, stack, tier);
}

/// A backpack's menu closed: what it holds goes back into the backpack.
pub(super) fn menu_closed(uuid: u128, window: i32, slots: &mut [Option<ItemStack>]) {
    TRASH.with(|t| t.borrow_mut().remove(&uuid));
    let ours = with(|s| s.open.get(&uuid).is_some_and(|&(_, _, w)| w == window));
    if ours {
        let items = slots.iter_mut().map(Option::take).collect();
        with(|s| s.closed.push((uuid, window, items)));
    }
}

/// Each server tick: the players' actions, the closed backpacks to fill (and
/// an equip or unequip waiting on one), who joined, and the backpacks to open.
pub(super) fn server_tick(game: &mut Game<'_>) {
    // Once a tick, whichever level ticks first.
    let now = game.world.game_time;
    let first = LAST_TICK.with(|t| t.replace(Some(now)) != Some(now));
    if first {
        // The config's tier sizes for the smithing table and the crafting grid.
        horizon_client::lan::containers::travelersbackpack_menu::set_tier_sizes([0, 1, 2, 3, 4].map(|n| {
            let t = configured_tier(n);
            [t.storage, t.upgrades, t.tools]
        }));
        stacked_sounds(game);
        loose_jukeboxes(game);
        backpack_slowness(game);
        smelt_open_menus(game);
        send_tag_lists(game);
        cap_actions();
    }
    // This level's own: its items, its players, its placed backpacks' menus.
    run_commands(game);
    protect_dropped(game);
    tick_worn(game);
    tick_abilities(game);
    sync_open_menus(game);
    tick_temporary_bags(game);
    write_closed_elsewhere(game);
    // A broken bag and its drops are one break: none is left for later.
    with(|s| s.bags_back.clear());
    let closed = with(|s| std::mem::take(&mut s.closed));
    for (uuid, window, items) in closed {
        let Some((source, dimension, open_window)) = with(|s| s.open.get(&uuid).cloned()) else {
            continue;
        };
        if open_window != window {
            continue;
        }
        let intact = with(|s| s.settings.contains_key(&uuid)) || menu_intact(game, uuid, source, &dimension);
        unmark_open(game, uuid, source);
        with(|s| {
            s.open.remove(&uuid);
            s.synced.remove(&uuid);
            s.access.remove(&uuid);
        });
        if !intact {
            with(|s| {
                s.extras.remove(&uuid);
                s.settings.remove(&uuid);
            });
            continue;
        }
        if with(|s| s.settings.remove(&uuid).is_some()) {
            with(|s| s.extras.remove(&uuid));
            if with(|s| s.reopen.remove(&uuid)) {
                with(|s| s.opening.push((uuid, source, dimension.clone())));
            }
            continue;
        }
        // The storage came back from the menu; the upgrades and tools follow it,
        // and what the fluid slots hold goes back to the player.
        let mut items = items;
        // The sizes it opened with (a backpack's own, from the config).
        let mut tier = tb::TIERS.into_iter().find(|t| t.storage == items.len()).unwrap_or(tb::TIERS[0]);
        if let Some((opened, extra)) = with(|s| s.extras.remove(&uuid)) {
            tier = opened;
            items.resize(tier.storage, None);
            items.extend(extra);
        }
        // The tabs' items stay in their upgrades; a tank's buckets go back.
        keep_tab_items(&mut items, tier);
        let tabs = shown_tabs(&items, tier);
        let blocks: Vec<Option<ItemStack>> = items.drain(tier.slots().min(items.len())..).collect();
        for (n, stack) in blocks.into_iter().enumerate() {
            if let Some(stack) = stack
                && tabs.get(n / tb::TAB_SLOTS) == Some(&tb::Tab::Tanks)
            {
                give_back(game, uuid, stack);
            }
        }
        if with(|s| s.reopen.remove(&uuid)) {
            with(|s| s.opening.push((uuid, source, dimension.clone())));
        }
        match source {
            Source::Hand(slot) => {
                let Some(host) = game.lan.as_mut() else {
                    continue;
                };
                let Some(g) = host.hub.guests.iter_mut().find(|g| g.placed && g.uuid == uuid) else {
                    continue;
                };
                if let Some(stack) = g.inventory.get_mut(slot).and_then(Option::as_mut)
                    && tb::is_backpack(&stack.item)
                {
                    set_contents(stack, items);
                }
                if with(|s| s.after_close.remove(&uuid)) == Some(Action::Equip) {
                    equip(game, uuid, slot);
                }
            }
            Source::Worn => {
                if let Some(mut stack) = worn(game, uuid) {
                    set_contents(&mut stack, items);
                    set_worn(game, uuid, Some(&stack));
                    tell_worn(game, uuid);
                }
                if with(|s| s.after_close.remove(&uuid)) == Some(Action::Unequip) {
                    unequip(game, uuid);
                }
            }
            Source::Other(owner) => {
                if let Some(mut stack) = worn(game, owner) {
                    set_contents(&mut stack, items);
                    set_worn(game, owner, Some(&stack));
                    tell_worn(game, owner);
                }
            }
            Source::Block(pos) => {
                if game.dim.id() != dimension {
                    // Written when this tick runs in its dimension.
                    with(|s| s.closed_elsewhere.push((pos, dimension.clone(), items)));
                    continue;
                }
                set_block_tanks(game, &dimension, pos, shows_tanks(&items, tier));
                game.native_with_mod_block_entity(pos, |m| {
                    if m.id != tb::BLOCK_ENTITY {
                        return;
                    }
                    for (i, item) in items.into_iter().enumerate() {
                        if let Some(slot) = m.items.get_mut(i) {
                            *slot = item;
                        }
                    }
                });
            }
        }
    }
    // After the menus closed (their contents back in their backpacks).
    let actions = with(|s| std::mem::take(&mut s.actions));
    for (uuid, action, dimension) in actions {
        act(game, uuid, action, &dimension);
    }
    // `AttachmentUtils.registerJoinEquip`: who joins learns what everyone
    // wears, and everyone what they wear.
    if !with(|s| std::mem::take(&mut s.joined)).is_empty() {
        let players: Vec<u128> =
            game.lan.as_ref().map(|h| h.hub.guests.iter().filter(|g| g.placed).map(|g| g.uuid).collect()).unwrap_or_default();
        for player in players {
            tell_worn(game, player);
        }
    }
    let opening = with(|s| std::mem::take(&mut s.opening));
    for (uuid, source, dimension) in opening {
        // Its backpack is open already (a click heard again): that menu stays.
        // An entry whose window the player no longer has is stale.
        if let Some(w) = with(|s| s.open.get(&uuid).map(|o| o.2)) {
            let showing = game.lan.as_ref().is_some_and(|h| {
                h.hub.guests.iter().any(|g| g.uuid == uuid && g.menu.as_ref().is_some_and(|m| m.window == w))
            });
            if showing {
                continue;
            }
            with(|s| {
                s.open.remove(&uuid);
                s.extras.remove(&uuid);
            });
        }
        let (items, tier, title, source, opened, bag) = match source {
            Source::Hand(slot) => {
                let Some(host) = game.lan.as_ref() else {
                    continue;
                };
                let Some(g) = host.hub.guests.iter().find(|g| g.placed && g.uuid == uuid) else {
                    continue;
                };
                // `usize::MAX`: the selected slot, the hand used.
                let slot = if slot == usize::MAX { g.selected } else { slot };
                let Some(stack) = g.inventory.get(slot).and_then(Option::as_ref).filter(|s| tb::is_backpack(&s.item))
                else {
                    continue;
                };
                let tier = stack_tier(stack);
                let title = screen_title(stack.data().and_then(|d| d.custom_name.clone()));
                (contents(stack, tier.slots()), tier, title, Source::Hand(slot), tb::Opened::Held, item_bag(stack))
            }
            Source::Worn => {
                let Some(stack) = worn(game, uuid) else {
                    continue;
                };
                let tier = stack_tier(&stack);
                let title = screen_title(stack.data().and_then(|d| d.custom_name.clone()));
                (contents(&stack, tier.slots()), tier, title, source, tb::Opened::Worn, item_bag(&stack))
            }
            Source::Other(owner) => {
                // Not while its wearer, or another, has it open.
                if with(|s| s.open.iter().any(|(v, o)| (*v == owner && o.0 == Source::Worn) || (*v != uuid && o.0 == source))) {
                    continue;
                }
                let Some(stack) = worn(game, owner) else {
                    continue;
                };
                let tier = stack_tier(&stack);
                let title = screen_title(stack.data().and_then(|d| d.custom_name.clone()));
                (contents(&stack, tier.slots()), tier, title, source, tb::Opened::Worn, item_bag(&stack))
            }
            Source::Block(pos) => {
                if game.dim.id() != dimension {
                    let there = game.lan.as_ref().is_some_and(|h| h.hub.guests.iter().any(|g| g.uuid == uuid && g.dimension == dimension));
                    if there {
                        with(|s| s.opening.push((uuid, source, dimension.clone())));
                    }
                    continue;
                }
                // One viewer at a time: each has their own copy of what it holds
                // (Java shares one live inventory), so two would duplicate it.
                // `preventMultiplePlayersAccess` is therefore always on.
                if with(|s| s.open.iter().any(|(u, o)| *u != uuid && o.0 == Source::Block(pos))) {
                    continue;
                }
                let block = game.native_block(pos);
                let Some(m) = game.native_mod_block_entity(pos).filter(|_| tb::is_backpack(&block)) else {
                    continue;
                };
                let tier = placed_tier(&m.nbt, &m.items);
                let mut items = m.items.clone();
                items.resize(tier.slots(), None);
                let title = screen_title(None);
                let bag = block_bag(&m.nbt);
                (items, tier, title, source, tb::Opened::Placed, bag)
            }
        };
        // The tabs' slots after the backpack's, each holding what its upgrade
        // kept; the screen learns which tabs show from the extra data.
        let mut items = items;
        items.resize(tier.slots(), None);
        load_tab_items(game, &mut items, tier);
        let settings = source_settings(game, uuid, source);
        let in_settings = with(|s| s.settings_enter.remove(&uuid));
        let mut data = if in_settings { vec![0; 5 * tier.upgrades] } else { menu_data(&items, tier) };
        let ui = in_settings.then(SettingsUi::default);
        data.extend(settings_data(&settings, ui, tier.storage));
        if !in_settings {
            data.extend(upgrade_masks(&items, tier));
            data.extend(ability_words(game, uuid, Some(source), &settings));
            data.extend(trash_words(uuid, tier.storage, tier.menu_slots()));
            data.extend(tank_colour_words(&items, tier));
        }
        let mut layout = if in_settings { vec![0; tier.upgrades] } else { layout_bytes(&items, tier) };
        // After the tabs: whether it has a bag, its tool slots show, it is the
        // settings screen; then the kept slots' items.
        layout.push(u8::from(bag >= 0));
        layout.push(u8::from(settings.tools));
        layout.push(u8::from(in_settings));
        for (slot, item, _) in &settings.memory {
            layout.extend(format!("{slot}={}\n", item.item).bytes());
        }
        let menu = tier.menu_for(opened);
        let extra = items[tier.storage..].to_vec();
        // The screen learns the sizes first; the slots past them hide.
        let mut sized = vec![tier.storage as u8, tier.upgrades as u8, tier.tools as u8];
        sized.append(&mut layout);
        let layout = sized;
        let hidden: Vec<usize> = (tier.menu_slots()..tb::MENU_SLOTS).collect();
        let mut items = items;
        items.resize(tb::MENU_SLOTS, None);
        // The settings screen: the storage to look at, nothing to take.
        let (items, input) = if in_settings {
            let mut shown = vec![None; items.len()];
            shown[..tier.storage].clone_from_slice(&items[..tier.storage]);
            (shown, 0)
        } else {
            (items, tier.storage)
        };
        let Some(host) = game.lan.as_mut() else {
            continue;
        };
        let Some(conn) = host.hub.guests.iter().find(|g| g.placed && g.uuid == uuid).map(|g| g.conn) else {
            continue;
        };
        let title = horizon_client::lan::data::component_nbt(&title.component());
        if let Some(window) = horizon_client::lan::containers::travelersbackpack_menu::open(
            &mut host.hub,
            conn,
            &menu,
            title,
            items,
            input,
            &layout,
            hidden,
        ) {
            if let Some(ui) = ui {
                with(|s| s.settings.insert(uuid, ui));
            } else {
                horizon_client::lan::containers::travelersbackpack_menu::set_rules(conn, window, move_rules(&settings, tier));
            }
            horizon_client::lan::containers::travelersbackpack_menu::send_data(&mut host.hub, conn, window, &data);
            with(|s| {
                s.open.insert(uuid, (source, dimension, window));
                s.extras.insert(uuid, (tier, extra));
            });
            mark_open(game, uuid, source);
        }
    }
}

/// The key, from a player's client (`ServerboundActionTagPacket.OPEN_SCREEN`).
pub(super) fn net_from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    if channel == TRASH_CHANNEL {
        if let [lo, hi, right] = *data {
            trash_slot(game, player, usize::from(u16::from_le_bytes([lo, hi])), right != 0);
        }
        return;
    }
    if channel == SORT_CHANNEL || channel == SORT_SHIFT_CHANNEL {
        if with(|s| s.open.contains_key(&player)) {
            sort_button(game, player, 0, channel == SORT_SHIFT_CHANNEL);
        }
        return;
    }
    if channel == ABILITY_TOGGLE_CHANNEL {
        if let Some(mut stack) = worn(game, player).filter(|s| ability(design_of(&s.item)).is_some()) {
            let mut settings = item_settings(&stack);
            settings.ability_off = !settings.ability_off;
            set_item_settings(&mut stack, &settings);
            set_worn(game, player, Some(&stack));
            let key = if settings.ability_off { "screen.travelersbackpack.ability_disabled" } else { "screen.travelersbackpack.ability_enabled" };
            let line = Text::literal(tr(game, key));
            game.native_action_bar(player, line);
        }
        return;
    }
    if let Some(i) = TOGGLE_UPGRADE_CHANNELS.iter().position(|c| *c == channel) {
        toggle_worn_upgrade(game, player, i);
        return;
    }
    if channel == HOSE_MODE_CHANNEL || channel == HOSE_TANK_CHANNEL {
        if let [value] = *data {
            hose_switch(game, player, channel == HOSE_TANK_CHANNEL, i32::from(value));
        }
        return;
    }
    if channel == TOOLS_CHANNEL {
        send_tools(game, player);
        return;
    }
    if channel == TOOL_SWAP_CHANNEL {
        if let [lo, hi, button] = *data {
            swap_tool(game, player, i32::from(i16::from_le_bytes([lo, hi])), button);
        }
        return;
    }
    // `allowOpeningFromSlot`: the key over a backpack in the inventory opens it.
    if channel == OPEN_SLOT_CHANNEL {
        if let [slot] = *data
            && cfg().allow_opening_from_slot
            && !with(|s| s.open.contains_key(&player))
        {
            let slot = usize::from(slot);
            let holds = game
                .lan
                .as_ref()
                .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == player))
                .and_then(|g| g.inventory.get(slot).cloned().flatten())
                .is_some_and(|s| tb::is_backpack(&s.item));
            if holds && slot < 36 {
                open_later_for(game, player, Source::Hand(slot));
            }
        }
        return;
    }
    if channel != OPEN_CHANNEL {
        return;
    }
    let dimension = game
        .lan
        .as_ref()
        .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == player))
        .map(|g| g.dimension.clone())
        .unwrap_or_else(|| game.dim.id().to_owned());
    with(|s| s.actions.push((player, Action::Open, dimension)));
}

/// A player died: their worn backpack is placed where they fell, at the
/// tick (the hub isn't in the game's hands here, the dying player's state
/// is: their dimension is the game's).
pub(super) fn player_died(game: &Game<'_>, player: u128) {
    let dimension = game.dim.id().to_owned();
    with(|s| s.actions.push((player, Action::Died, dimension)));
}

/// A player joined: they and everyone learn what each wears.
pub(super) fn logged_in(player: u128) {
    with(|s| s.joined.push(player));
}

/// A button of a backpack's menu (`ServerboundActionTagPacket.EQUIP_BACKPACK`):
/// 0 equips the backpack opened from the inventory, 1 unequips the worn one.
pub(super) fn menu_button(host: &super::lan::LanHost, conn: u32, window: i32, id: i32) {
    let Some(g) = host.hub.guest(conn) else {
        return;
    };
    let ours = with(|s| s.open.get(&g.uuid).is_some_and(|o| o.2 == window));
    let action = match id {
        0 => Action::Equip,
        1 => Action::Unequip,
        2 => Action::SleepingBag,
        3 => Action::DetachBag,
        30..=33 => Action::Sort(id as u8 - 30, false),
        34..=37 => Action::Sort(id as u8 - 34, true),
        40 => Action::ToggleTools,
        41 => Action::ToggleMore,
        42 => Action::Settings(true),
        43 => Action::Settings(false),
        44 => Action::SettingsTab(1),
        45 => Action::SettingsTab(2),
        46 => Action::SelectAll(true),
        47 => Action::SelectAll(false),
        48 => Action::ToggleMatch,
        49 => Action::ToggleVisible,
        90 => Action::AbilitySlider,
        95 => Action::TrashBin,
        96 | 97 => Action::TankClick(id as usize - 96),
        50..=55 => Action::RemoveUpgrade(id as usize - 50),
        60..=77 => Action::FilterButton((id as usize - 60) / 3, (id as usize - 60) % 3),
        80..=85 => Action::ToggleEnabled(id as usize - 80),
        200..=499 => Action::FilterTag((id as usize - 200) / 50, (id as usize - 200) % 50),
        10..=15 => Action::ToggleTab(id as usize - 10),
        20..=25 => Action::Jukebox(id as usize - 20),
        _ => return,
    };
    if ours {
        let dimension = g.dimension.clone();
        with(|s| s.actions.push((g.uuid, action, dimension)));
    }
}

// ---- the sleeping bag (`SleepingBagBlock`, `BackpackBlockEntity.deploySleepingBag`) ----------

/// `DyeColor` by id: the bags' colours.
const BAG_COLOURS: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue",
    "brown", "green", "red", "black",
];
/// The bag a backpack starts with (`SleepingBagItem.getDefaultColor`); -1 is none.
const RED: i32 = 14;
/// A backpack item's bag (`SLEEPING_BAG_COLOR`): its `minecraft:custom_data`,
/// under this key, red when unset.
const BAG_COLOUR_KEY: &str = "TravelersBackpackSleepingBag";
/// A placed backpack's, in its block entity's compound.
const BAG_COLOUR_NBT: &str = "SleepingBagColor";

fn item_bag(stack: &ItemStack) -> i32 {
    match custom_data(stack).get(&horizon_world::nbt::JavaString::from(BAG_COLOUR_KEY)) {
        Some(Tag::Int(n)) => *n,
        _ => RED,
    }
}

fn set_item_bag(stack: &mut ItemStack, bag: i32) {
    let mut data = custom_data(stack);
    let key = horizon_world::nbt::JavaString::from(BAG_COLOUR_KEY);
    if bag == RED {
        data.remove(&key);
    } else {
        data.insert(key, Tag::Int(bag));
    }
    set_custom_data(stack, data);
    refresh_look(stack);
}

fn block_bag(nbt: &Compound) -> i32 {
    match nbt.get(&horizon_world::nbt::JavaString::from(BAG_COLOUR_NBT)) {
        Some(Tag::Int(n)) => *n,
        _ => RED,
    }
}

/// A colour's name, `none` for no bag: the block's `bag` state.
fn bag_name(bag: i32) -> &'static str {
    usize::try_from(bag).ok().and_then(|i| BAG_COLOURS.get(i)).copied().unwrap_or("none")
}

/// The sleeping bag of a colour (`getProperSleepingBag`).
fn bag_block(bag: i32) -> Option<String> {
    usize::try_from(bag).ok().and_then(|i| BAG_COLOURS.get(i)).map(|c| format!("travelersbackpack:{c}_sleeping_bag"))
}

/// The colour of sleeping bag `item` (`ShapedBackpackRecipe.getProperColor`).
fn bag_colour_of(item: &str) -> Option<i32> {
    let colour = item.strip_prefix("travelersbackpack:")?.strip_suffix("_sleeping_bag")?;
    BAG_COLOURS.iter().position(|c| *c == colour).map(|i| i as i32)
}

/// Into the player's inventory, else at their feet.
fn give_or_drop(game: &mut Game<'_>, uuid: u128, stack: ItemStack) {
    let left = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)).and_then(|g| {
        match free_slot(&g.inventory) {
            Some(slot) => {
                g.inventory[slot] = Some(stack);
                None
            }
            None => Some((stack, g.position)),
        }
    });
    if let Some((stack, at)) = left {
        game.spawn_item(stack, at, [0.0; 3], 10);
    }
}

/// `RightClickHandler`, changing the sleeping bag: the one held takes the
/// placed backpack's bag's place, which drops.
fn change_bag(game: &mut Game<'_>, uuid: u128, dimension: &str, pos: [i32; 3]) {
    if !tb::is_backpack(&game.native_block(pos)) {
        return;
    }
    let Some(host) = game.lan.as_mut() else {
        return;
    };
    let Some(g) = host.hub.guests.iter_mut().find(|g| g.placed && g.uuid == uuid) else {
        return;
    };
    let selected = g.selected;
    let Some(new) = g.inventory.get(selected).and_then(Option::as_ref).and_then(|s| bag_colour_of(&s.item)) else {
        return;
    };
    if let Some(held) = g.inventory[selected].as_mut() {
        held.count -= 1;
        if held.count <= 0 {
            g.inventory[selected] = None;
        }
    }
    fold_bag_of(game, dimension, pos);
    let old = game.native_mod_block_entity(pos).map_or(RED, |m| block_bag(&m.nbt));
    game.native_with_mod_block_entity(pos, |m| {
        m.nbt.insert(BAG_COLOUR_NBT.into(), Tag::Int(new));
    });
    let tanks = game
        .native_mod_block_entity(pos)
        .is_some_and(|m| shows_tanks(&m.items, placed_tier(&m.nbt, &m.items)));
    set_block_tanks(game, dimension, pos, tanks);
    if let Some(old) = bag_block(old) {
        let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.0, f64::from(pos[2]) + 0.5];
        game.spawn_item(ItemStack::new(&old, 1), at, super::dispenser::new_item_motion(), 10);
    }
    advance(game, uuid, "change_sleeping_bag");
}

/// `ServerActions.toggleSleepingBag` with shift: the bag comes off into the
/// inventory, the menu closes; the backpack has none until one is put on.
fn detach_bag(game: &mut Game<'_>, uuid: u128, dimension: &str, source: Source, window: i32) {
    let bag = match source {
        Source::Block(pos) => game.native_mod_block_entity(pos).map_or(-1, |m| block_bag(&m.nbt)),
        Source::Worn => worn(game, uuid).map_or(-1, |s| item_bag(&s)),
        Source::Hand(_) | Source::Other(_) => -1,
    };
    let Some(item) = bag_block(bag) else {
        return;
    };
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    if let Some(host) = game.lan.as_mut() {
        horizon_client::lan::containers::travelersbackpack_menu::close(&mut host.hub, conn, window);
    }
    match source {
        Source::Block(pos) => {
            fold_bag_of(game, dimension, pos);
            game.native_with_mod_block_entity(pos, |m| {
                m.nbt.insert(BAG_COLOUR_NBT.into(), Tag::Int(-1));
            });
            let tanks = game
                .native_mod_block_entity(pos)
                .is_some_and(|m| shows_tanks(&m.items, placed_tier(&m.nbt, &m.items)));
            set_block_tanks(game, dimension, pos, tanks);
        }
        Source::Worn => {
            let Some(mut stack) = worn(game, uuid) else {
                return;
            };
            set_item_bag(&mut stack, -1);
            set_worn(game, uuid, Some(&stack));
            tell_worn(game, uuid);
        }
        Source::Hand(_) | Source::Other(_) => return,
    }
    give_or_drop(game, uuid, ItemStack::new(&item, 1));
}
/// A placed backpack's block entity: whether its bag is laid out in front.
const BAG_DEPLOYED: &str = "SleepingBag";

fn is_sleeping_bag(block: &str) -> bool {
    block.strip_prefix("travelersbackpack:").is_some_and(|n| n.ends_with("_sleeping_bag"))
}

/// A horizontal direction's step.
fn step(facing: &str) -> [i32; 3] {
    match facing {
        "south" => [0, 0, 1],
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        _ => [0, 0, -1],
    }
}

fn add(a: [i32; 3], b: [i32; 3], times: i32) -> [i32; 3] {
    [a[0] + b[0] * times, a[1] + b[1] * times, a[2] + b[2] * times]
}

/// `Direction.fromYRot`: the way a player looks along the ground.
fn facing_of_yaw(yaw: f32) -> &'static str {
    match ((yaw / 90.0 + 0.5).floor() as i32).rem_euclid(4) {
        0 => "south",
        1 => "west",
        2 => "north",
        _ => "east",
    }
}

/// `canPlaceSleepingBag` and the ground under its foot: not air, not a liquid.
fn bag_fits(game: &Game<'_>, foot: [i32; 3], head: [i32; 3]) -> bool {
    let below = game.native_block([foot[0], foot[1] - 1, foot[2]]);
    replaceable(&game.native_block(foot))
        && replaceable(&game.native_block(head))
        && !below.ends_with("air")
        && below != "minecraft:water"
        && below != "minecraft:lava"
}

/// Lays the bag out, its foot at `foot` and its head a step `facing` on.
fn place_bag(game: &mut Game<'_>, dimension: &str, foot: [i32; 3], facing: &str, bag: i32) -> bool {
    let Some(bag) = bag_block(bag) else {
        return false;
    };
    let head = add(foot, step(facing), 1);
    if !bag_fits(game, foot, head) {
        return false;
    }
    let registry = game.pack.assets.registry;
    let Some(state) = game.native_default_state(&bag) else {
        return false;
    };
    let part = |part: &str| {
        let mut s = registry.state(state)?.clone();
        s.properties.insert("facing".into(), facing.into());
        s.properties.insert("part".into(), part.into());
        s.properties.insert("occupied".into(), "false".into());
        registry.resolve(&s).ok()
    };
    let (Some(f), Some(h)) = (part("foot"), part("head")) else {
        return false;
    };
    game.native_set_block(dimension, foot, f, 3) && game.native_set_block(dimension, head, h, 3)
}

/// Takes the bag at `foot`..`head` away, when it is one.
fn remove_bag(game: &mut Game<'_>, dimension: &str, foot: [i32; 3], head: [i32; 3]) {
    let Some(air) = game.native_default_state("minecraft:air") else {
        return;
    };
    for pos in [head, foot] {
        if is_sleeping_bag(&game.native_block(pos)) {
            game.native_set_block(dimension, pos, air, 3);
        }
    }
}

/// The way a placed backpack's front faces (`FACING`, toward who placed it).
fn backpack_front(game: &Game<'_>, pos: [i32; 3]) -> &'static str {
    match game.native_block_property(pos, "facing").as_deref() {
        Some("south") => "south",
        Some("east") => "east",
        Some("west") => "west",
        _ => "north",
    }
}

/// Whether placed backpack `pos` has its bag out.
fn bag_deployed(game: &Game<'_>, pos: [i32; 3]) -> bool {
    game.native_mod_block_entity(pos)
        .is_some_and(|m| matches!(m.nbt.get(&horizon_world::nbt::JavaString::from(BAG_DEPLOYED)), Some(Tag::Byte(1))))
}

fn set_bag_deployed(game: &mut Game<'_>, pos: [i32; 3], deployed: bool) {
    game.native_with_mod_block_entity(pos, |m| {
        m.nbt.insert(BAG_DEPLOYED.into(), Tag::Byte(i8::from(deployed)));
    });
}

/// A placed backpack goes (broken, picked up, worn): its bag goes with it.
fn fold_bag_of(game: &mut Game<'_>, dimension: &str, pos: [i32; 3]) {
    if bag_deployed(game, pos) {
        let foot = add(pos, step(backpack_front(game, pos)), 1);
        let head = add(pos, step(backpack_front(game, pos)), 2);
        remove_bag(game, dimension, foot, head);
        set_bag_deployed(game, pos, false);
    }
}

/// The sleeping bag button (`ServerActions.toggleSleepingBag`): a placed
/// backpack lays its bag out in front, or folds it; worn, the bag goes down
/// where the player stands and they lie in it (`placeAndUseSleepingBag`),
/// taken up again when they get up. The menu closes either way.
fn use_sleeping_bag(game: &mut Game<'_>, uuid: u128, dimension: &str, source: Source, window: i32) {
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    if let Some(host) = game.lan.as_mut() {
        horizon_client::lan::containers::travelersbackpack_menu::close(&mut host.hub, conn, window);
    }
    match source {
        Source::Block(pos) => {
            let tanks = game
                .native_mod_block_entity(pos)
                .is_some_and(|m| shows_tanks(&m.items, placed_tier(&m.nbt, &m.items)));
            if bag_deployed(game, pos) {
                fold_bag_of(game, dimension, pos);
                set_block_tanks(game, dimension, pos, tanks);
            } else if place_bag(
                game,
                dimension,
                add(pos, step(backpack_front(game, pos)), 1),
                backpack_front(game, pos),
                game.native_mod_block_entity(pos).map_or(RED, |m| block_bag(&m.nbt)),
            ) {
                set_bag_deployed(game, pos, true);
                set_block_tanks(game, dimension, pos, tanks);
            }
        }
        Source::Worn => {
            let Some((position, yaw)) =
                game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| (g.position, g.yaw))
            else {
                return;
            };
            let foot = position.map(|c| c.floor() as i32);
            let facing = facing_of_yaw(yaw);
            let head = add(foot, step(facing), 1);
            let bag = worn(game, uuid).map_or(-1, |s| item_bag(&s));
            if !place_bag(game, dimension, foot, facing, bag) {
                return;
            }
            advance(game, uuid, "use_sleeping_bag");
            let Some(mut host) = game.lan.take() else {
                return;
            };
            game.as_guest(&mut host, conn, |g| g.try_sleep(head));
            let asleep = host.hub.guests.iter().any(|g| g.uuid == uuid && g.bed.is_some());
            game.lan = Some(host);
            if asleep {
                with(|s| s.temporary_bags.insert(uuid, (foot, head, dimension.to_owned())));
                save_temporary_bags(game);
            } else {
                // Day, monsters, no room: no sleep, no bag.
                remove_bag(game, dimension, foot, head);
            }
        }
        Source::Hand(_) | Source::Other(_) => {}
    }
}

/// A worn backpack's bag goes once its sleeper is up (`SleepHandler`).
fn tick_temporary_bags(game: &mut Game<'_>) {
    load_temporary_bags(game);
    let dimension = game.dim.id().to_owned();
    let bags: Vec<(u128, ([i32; 3], [i32; 3], String))> =
        with(|s| s.temporary_bags.iter().map(|(u, b)| (*u, b.clone())).collect());
    for (uuid, (foot, head, dim)) in bags {
        if dim != dimension {
            continue;
        }
        let asleep = game
            .lan
            .as_ref()
            .is_some_and(|h| h.hub.guests.iter().any(|g| g.uuid == uuid && g.bed == Some(head)));
        // One left from before a restart waits for its chunk to load.
        let loaded = [foot, head].iter().all(|p| game.world.chunks.contains_key(&(p[0] >> 4, p[2] >> 4)));
        if !asleep && loaded {
            remove_bag(game, &dimension, foot, head);
            with(|s| s.temporary_bags.remove(&uuid));
            save_temporary_bags(game);
        }
    }
}

thread_local! {
    /// The world whose temporary bags were read.
    static TEMPORARY_BAGS_READ: RefCell<Option<std::path::PathBuf>> = const { RefCell::new(None) };
}

/// The laid-out bags of worn backpacks, kept in the world (the original keeps
/// them as blocks that drop nothing, `CAN_DROP`): a server stopped with one
/// out finds it at the next start and takes it up, nothing dropped.
fn temporary_bags_file(game: &Game<'_>) -> Option<std::path::PathBuf> {
    Some(game.native_world_dir()?.join("data").join("travelersbackpack_temporary_bags.txt"))
}

fn load_temporary_bags(game: &Game<'_>) {
    let Some(file) = temporary_bags_file(game) else {
        return;
    };
    if TEMPORARY_BAGS_READ.with(|r| r.borrow().as_ref() == Some(&file)) {
        return;
    }
    TEMPORARY_BAGS_READ.with(|r| *r.borrow_mut() = Some(file.clone()));
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let bags: Vec<(u128, ([i32; 3], [i32; 3], String))> = text
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let [uuid, fx, fy, fz, hx, hy, hz, dimension] = parts[..] else {
                return None;
            };
            let n = |t: &str| t.parse::<i32>().ok();
            Some((u128::from_str_radix(uuid, 16).ok()?, ([n(fx)?, n(fy)?, n(fz)?], [n(hx)?, n(hy)?, n(hz)?], dimension.to_owned())))
        })
        .collect();
    with(|s| {
        s.temporary_bags.clear();
        s.temporary_bags.extend(bags);
    });
}

fn save_temporary_bags(game: &Game<'_>) {
    let Some(file) = temporary_bags_file(game) else {
        return;
    };
    let text: String = with(|s| {
        s.temporary_bags
            .iter()
            .map(|(u, (f, h, d))| format!("{u:032x} {} {} {} {} {} {} {d}
", f[0], f[1], f[2], h[0], h[1], h[2]))
            .collect()
    });
    if text.is_empty() {
        let _ = std::fs::remove_file(&file);
    } else if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(&file, text);
    }
}

/// The placed backpack a laid-out bag at `pos` belongs to, if it is one's.
fn bag_owner(game: &Game<'_>, pos: [i32; 3], state: horizon_world::registry::BlockStateId) -> Option<[i32; 3]> {
    let (facing, part) = game.pack.assets.registry.state(state).map(|s| {
        let get = |k: &str| s.properties.get(k).cloned().unwrap_or_default();
        (get("facing"), get("part"))
    })?;
    let head = if part == "head" { pos } else { add(pos, step(&facing), 1) };
    let backpack = add(head, step(&facing), -2);
    (tb::is_backpack(&game.native_block(backpack)) && bag_deployed(game, backpack)).then_some(backpack)
}

/// A sleeping bag being broken (`SleepingBagBlock.playerWillDestroy`, in
/// creative too): one a backpack laid out is rolled up on it again.
fn bag_broken(game: &mut Game<'_>, pos: [i32; 3]) {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let Some(backpack) = bag_owner(game, pos, state) else {
        return;
    };
    set_bag_deployed(game, backpack, false);
    let dimension = game.dim.id().to_owned();
    with(|s| {
        s.bags_back.insert(backpack);
        s.actions.push((0, Action::BlockTanks(backpack), dimension));
    });
}

/// A sleeping bag broken: a bag a backpack laid out comes back to it (no
/// drop, `CAN_DROP` false), one put down as an item drops as one.
fn sleeping_bag_drops(game: &mut Game<'_>, pos: [i32; 3], state: horizon_world::registry::BlockStateId, drops: &mut Vec<ItemStack>) {
    let Some((block, facing, part)) = game.pack.assets.registry.state(state).map(|s| {
        let get = |k: &str| s.properties.get(k).cloned().unwrap_or_default();
        (s.name.to_string(), get("facing"), get("part"))
    }) else {
        return;
    };
    if !is_sleeping_bag(&block) {
        return;
    }
    let block = block.as_str();
    let head = if part == "head" { pos } else { add(pos, step(&facing), 1) };
    let backpack = add(head, step(&facing), -2);
    let from_backpack = with(|s| s.bags_back.remove(&backpack))
        || (tb::is_backpack(&game.native_block(backpack)) && bag_deployed(game, backpack));
    let temporary = with(|s| s.temporary_bags.values().any(|b| b.1 == head));
    if from_backpack {
        set_bag_deployed(game, backpack, false);
        // Rolled up on the backpack again, at the tick.
        let dimension = game.dim.id().to_owned();
        with(|s| s.actions.push((0, Action::BlockTanks(backpack), dimension)));
        return;
    }
    if !temporary && part == "head" && !drops.iter().any(|d| &*d.item == block) {
        drops.push(ItemStack::new(block, 1));
    }
}

// ---- the upgrade and tool slots -------------------------------------------------------------

/// A backpack upgrade (`UpgradeItem`): the ones its upgrade slots take, the
/// tier upgrades and the blank one not among them.
fn is_upgrade(id: &str) -> bool {
    id.strip_prefix("travelersbackpack:")
        .is_some_and(|n| n.ends_with("_upgrade") && !n.ends_with("_tier_upgrade") && n != "blank_upgrade")
}

/// `ToolSlotItemHandler.isValid`: a tool, a weapon, anything that doesn't
/// stack, or one of the tag's; never a backpack, a bundle or a shulker box.
fn fits_tool_slot(game: &Game<'_>, stack: &ItemStack) -> bool {
    let id = &*stack.item;
    if tb::is_backpack(id) || id.ends_with("bundle") || id.ends_with("shulker_box") || id == "travelersbackpack:hose" {
        return false;
    }
    if cfg().tool_slots_accept_everything || cfg().tool_slots_acceptable_items.iter().any(|i| i == id) {
        return true;
    }
    let data = stack.data();
    game.items().get(id).is_some_and(|i| i.max_stack == 1)
        || game.items().item_in_tag("travelersbackpack:acceptable_tools", id)
        || data.is_some_and(|d| d.other("minecraft:tool").is_some() || d.other("minecraft:weapon").is_some())
}


/// A click on a backpack menu's upgrade, tool or tab slot
/// (`UpgradeSlotItemHandler`, `ToolSlotItemHandler`, the tabs' slots): the
/// cursor's stack goes in when the slot takes it (an upgrade one at a time,
/// and one of each kind), what is there comes out; a shift-click moves it
/// into the inventory. A tab then does its work (the tanks pour, the
/// crafting grid shows its result); putting an upgrade with a tab in or
/// taking it out opens the menu again, with its tabs as they are now.
pub(super) fn menu_clicked(game: &mut Game<'_>, uuid: u128, window: i32, slot: i32, button: i32, mode: i32) {
    let ours = with(|s| s.open.get(&uuid).is_some_and(|o| o.2 == window));
    let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
        return;
    };
    let Ok(slot) = usize::try_from(slot) else {
        return;
    };
    if ours && with(|s| s.settings.contains_key(&uuid)) {
        if slot < tier.storage {
            settings_click(game, uuid, slot, button, mode);
        }
        return;
    }
    if !ours || slot < tier.storage || slot >= tier.menu_slots() {
        return;
    }
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    let Some((mut items, mut carried)) =
        game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
    else {
        return;
    };
    if items.len() < tier.menu_slots() {
        return;
    }
    let tabs_before = shown_tabs(&items, tier);
    let upgrades = tier.storage..tier.storage + tier.upgrades;
    // The tabs' items go into their upgrades first: an upgrade taken out
    // takes them along.
    keep_tab_items(&mut items, tier);
    // An upgrade slot whose upgrade changed: the old one's tab block empties
    // (it is in the upgrade now), the new one's comes with the menu again.
    let mut swapped = false;
    if upgrades.contains(&slot) {
        let i = slot - tier.storage;
        let before = items[slot].clone();
        click_upgrade(game, uuid, &mut items, &mut carried, tier, slot, mode);
        if items[slot].as_ref().map(|s| &s.item) != before.as_ref().map(|s| &s.item) {
            let old = tb::Tab::of(before.as_ref().map(|s| &*s.item));
            let shown = before.as_ref().filter(|s| tab_open(s)).map_or(tb::Tab::Plain, |_| old);
            empty_tab_block(game, uuid, &mut items, tier, i, shown);
            swapped = old != tb::Tab::Plain || tabs_of(&items, tier)[i] != tb::Tab::Plain;
        }
    } else if slot < tier.slots() {
        click_plain(game, uuid, &mut items, &mut carried, slot, button, mode, |game, s| fits_tool_slot(game, s));
    } else {
        let i = (slot - tier.slots()) / tb::TAB_SLOTS;
        let k = (slot - tier.slots()) % tb::TAB_SLOTS;
        click_tab(game, uuid, &mut items, &mut carried, tier, tabs_before[i], i, k, button, mode);
    }
    work_tabs(game, &mut items, tier);
    let tabs_now = shown_tabs(&items, tier);
    with(|s| s.extras.insert(uuid, (tier, items[tier.storage..].to_vec())));
    let data = full_data(game, uuid, &items, tier);
    if let Some(host) = game.lan.as_mut() {
        let hub = &mut host.hub;
        horizon_client::lan::containers::travelersbackpack_menu::set_items(hub, conn, items, carried);
        horizon_client::lan::containers::travelersbackpack_menu::send_data(hub, conn, window, &data);
        // The menu with its tabs as they are: it closes (its contents
        // written back) and opens again.
        if (tabs_now != tabs_before || swapped) && horizon_client::lan::containers::travelersbackpack_menu::close(hub, conn, window) {
            with(|s| s.reopen.insert(uuid));
        }
    }
}

/// An upgrade slot: one upgrade, one of each kind.
fn click_upgrade(
    game: &mut Game<'_>,
    uuid: u128,
    items: &mut [Option<ItemStack>],
    carried: &mut Option<ItemStack>,
    tier: tb::Tier,
    slot: usize,
    mode: i32,
) {
    let upgrades = tier.storage..tier.storage + tier.upgrades;
    if mode == 0
        && carried.is_none()
        && let Some(upgrade) = items[slot].as_mut()
    {
        let tab = tb::Tab::of(Some(&upgrade.item));
        if tab != tb::Tab::Plain {
            let open = tab_open(upgrade);
            set_tab_open(upgrade, !open);
            if open {
                empty_tab_block(game, uuid, items, tier, slot - tier.storage, tab);
            }
            return;
        }
    }
    let current = items[slot].take();
    match mode {
        0 => match (carried.take(), current) {
            (None, current) => *carried = current,
            (Some(held), current) => {
                let mut fits = is_upgrade(&held.item)
                    && !upgrades.clone().any(|k| k != slot && items[k].as_ref().is_some_and(|s| s.item == held.item));
                // `TanksUpgradeItem.canBePutInBackpack`: more fluid than this tier's tanks hold.
                if fits && &*held.item == TANKS_UPGRADE {
                    let over: Vec<String> = tanks_of(&held)
                        .iter()
                        .filter(|t| t.1 > tank_capacity(tier))
                        .map(|t| format!("{}\t{}\t{}", t.0, mb(t.1), mb(tank_capacity(tier))))
                        .collect();
                    if !over.is_empty() {
                        fits = false;
                        let _ = game.native_net_send_to_player(uuid, FLUID_WARNING_CHANNEL, over.join("\n").as_bytes());
                    }
                }
                match (fits, current) {
                    (false, current) => {
                        items[slot] = current;
                        *carried = Some(held);
                    }
                    (true, None) if held.count > 1 => {
                        let mut one = held.clone();
                        one.count = 1;
                        items[slot] = Some(one);
                        let mut rest = held;
                        rest.count -= 1;
                        *carried = Some(rest);
                    }
                    (true, Some(current)) if held.count > 1 => {
                        items[slot] = Some(current);
                        *carried = Some(held);
                    }
                    (true, current) => {
                        items[slot] = Some(held);
                        *carried = current;
                    }
                }
            }
        },
        1 => items[slot] = shift_out(game, uuid, current),
        _ => items[slot] = current,
    }
}

/// A slot holding what `fits` lets in, by vanilla's clicks: the left button
/// picks up, puts down, stacks or swaps; the right one takes half or puts
/// one down; shift moves the stack into the inventory.
#[allow(clippy::too_many_arguments)]
fn click_plain(
    game: &mut Game<'_>,
    uuid: u128,
    items: &mut [Option<ItemStack>],
    carried: &mut Option<ItemStack>,
    slot: usize,
    button: i32,
    mode: i32,
    fits: impl Fn(&Game<'_>, &ItemStack) -> bool,
) {
    let current = items[slot].take();
    match mode {
        0 => {
            let max = |game: &Game<'_>, s: &ItemStack| game.items().get(&s.item).map_or(64, |i| i.max_stack.max(1));
            match (carried.take(), current, button) {
                (None, None, _) => {}
                (None, Some(mut here), 1) => {
                    let half = here.count.div_ceil(2);
                    let mut taken = here.clone();
                    taken.count = half;
                    here.count -= half;
                    items[slot] = (here.count > 0).then_some(here);
                    *carried = Some(taken);
                }
                (None, here, _) => *carried = here,
                (Some(held), here, _) if !fits(game, &held) => {
                    items[slot] = here;
                    *carried = Some(held);
                }
                (Some(mut held), None, 1) => {
                    let mut one = held.clone();
                    one.count = 1;
                    items[slot] = Some(one);
                    held.count -= 1;
                    *carried = (held.count > 0).then_some(held);
                }
                (Some(mut held), Some(mut here), b) if same_item(&held, &here) => {
                    let room = max(game, &here).saturating_sub(here.count);
                    let moved = if b == 1 { room.min(1) } else { room.min(held.count) };
                    here.count += moved;
                    held.count -= moved;
                    items[slot] = Some(here);
                    *carried = (held.count > 0).then_some(held);
                }
                (Some(held), here, _) => {
                    items[slot] = Some(held);
                    *carried = here;
                }
            }
        }
        1 => items[slot] = shift_out(game, uuid, current),
        _ => items[slot] = current,
    }
}

/// A shift-click's stack into the player's inventory: what had no room.
fn shift_out(game: &mut Game<'_>, uuid: u128, stack: Option<ItemStack>) -> Option<ItemStack> {
    let stack = stack?;
    let g = game.lan.as_mut()?.hub.guests.iter_mut().find(|g| g.uuid == uuid)?;
    match free_slot(&g.inventory) {
        Some(k) => {
            g.inventory[k] = Some(stack);
            None
        }
        None => Some(stack),
    }
}

/// A click on slot `k` of upgrade slot `i`'s tab.
#[allow(clippy::too_many_arguments)]
fn click_tab(
    game: &mut Game<'_>,
    uuid: u128,
    items: &mut [Option<ItemStack>],
    carried: &mut Option<ItemStack>,
    tier: tb::Tier,
    tab: tb::Tab,
    i: usize,
    k: usize,
    button: i32,
    mode: i32,
) {
    use tb::Tab;
    if k >= tab.slots().len() {
        return;
    }
    let base = tier.slots() + i * tb::TAB_SLOTS;
    let slot = base + k;
    match tab {
        Tab::Plain => {}
        Tab::Tanks => match k {
            // A tank's input takes a bucket; its output only gives.
            0 | 2 => click_plain(game, uuid, items, carried, slot, button, mode, |_, s| container_of(s).is_some()),
            _ => click_plain(game, uuid, items, carried, slot, button, mode, |_, _| false),
        },
        Tab::Crafting if k == 9 => take_crafted(game, uuid, items, carried, base, mode == 1),
        Tab::Crafting => click_plain(game, uuid, items, carried, slot, button, mode, |_, _| true),
        Tab::Furnace | Tab::Smoker | Tab::BlastFurnace => match k {
            0 => click_plain(game, uuid, items, carried, slot, button, mode, |_, _| true),
            1 => click_plain(game, uuid, items, carried, slot, button, mode, |game, s| fuel_ticks(game, s) > 0),
            _ => click_plain(game, uuid, items, carried, slot, button, mode, |_, _| false),
        },
        // `FilterSlotItemHandler`: a copy of what the cursor holds, none taken.
        Tab::Void | Tab::Refill | Tab::Pickup | Tab::Magnet | Tab::Feeding => {
            items[slot] = carried.as_ref().map(|c| {
                let mut ghost = c.clone();
                ghost.count = 1;
                ghost
            });
        }
        // `JukeboxUpgrade`: a disc, not taken out while it plays.
        Tab::Jukebox => {
            if items[tier.storage + i].as_ref().and_then(playing).is_none() {
                click_plain(game, uuid, items, carried, slot, button, mode, |_, s| game_has_song(s));
            }
        }
    }
}

/// A music disc (`DataComponents.JUKEBOX_PLAYABLE`).
fn game_has_song(stack: &ItemStack) -> bool {
    stack.item.starts_with("minecraft:music_disc_")
}

/// The tab each upgrade slot shows.
/// Whether an upgrade's tab is open (`ModDataComponents.TAB_OPEN`, closed as it
/// comes): an open one shows its slots.
fn tab_open(stack: &ItemStack) -> bool {
    matches!(custom_data(stack).get(&horizon_world::nbt::JavaString::from(TAB_OPEN_KEY)), Some(Tag::Byte(1)))
}

fn set_tab_open(stack: &mut ItemStack, open: bool) {
    let mut data = custom_data(stack);
    if open {
        data.insert(TAB_OPEN_KEY.into(), Tag::Byte(1));
    } else {
        data.remove(&horizon_world::nbt::JavaString::from(TAB_OPEN_KEY));
    }
    set_custom_data(stack, data);
}

const TAB_OPEN_KEY: &str = "TravelersBackpackTabOpen";

/// The toggle of upgrade slot `i`'s tab (`TabWidget`): it opens or closes,
/// and the menu opens again to show it so.
fn toggle_tab(game: &mut Game<'_>, uuid: u128, window: i32, i: usize) {
    let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
        return;
    };
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    let Some((mut items, carried)) =
        game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
    else {
        return;
    };
    if items.len() < tier.menu_slots() || i >= tier.upgrades {
        return;
    }
    keep_tab_items(&mut items, tier);
    let Some(upgrade) = items[tier.storage + i].as_mut() else {
        return;
    };
    let tab = tb::Tab::of(Some(&upgrade.item));
    if tab == tb::Tab::Plain {
        return;
    }
    let open = tab_open(upgrade);
    set_tab_open(upgrade, !open);
    if open {
        // Closing: its slots' items are in the upgrade; a tank's buckets go back.
        empty_tab_block(game, uuid, &mut items, tier, i, tab);
    }
    with(|s| s.extras.insert(uuid, (tier, items[tier.storage..].to_vec())));
    if let Some(host) = game.lan.as_mut() {
        let hub = &mut host.hub;
        horizon_client::lan::containers::travelersbackpack_menu::set_items(hub, conn, items, carried);
        if horizon_client::lan::containers::travelersbackpack_menu::close(hub, conn, window) {
            with(|s| s.reopen.insert(uuid));
        }
    }
}

/// Whether a jukebox upgrade plays (`ModDataComponents.IS_PLAYING`): the
/// sound event it plays.
const PLAYING_KEY: &str = "TravelersBackpackPlaying";

fn playing(stack: &ItemStack) -> Option<String> {
    match custom_data(stack).get(&horizon_world::nbt::JavaString::from(PLAYING_KEY)) {
        Some(Tag::String(s)) => s.to_string().ok(),
        _ => None,
    }
}

fn set_playing(stack: &mut ItemStack, sound: Option<&str>) {
    let mut data = custom_data(stack);
    match sound {
        Some(sound) => {
            data.insert(PLAYING_KEY.into(), Tag::String(sound.into()));
        }
        None => {
            data.remove(&horizon_world::nbt::JavaString::from(PLAYING_KEY));
        }
    }
    set_custom_data(stack, data);
}

/// A music disc's song (`music_disc_cat` plays `music_disc.cat`).
fn disc_sound(disc: &ItemStack) -> Option<String> {
    let name = disc.item.strip_prefix("minecraft:music_disc_")?;
    Some(format!("minecraft:music_disc.{name}"))
}

/// The jukebox tab's button (`JukeboxWidget`): its disc plays where the
/// backpack is (placed, at its block; carried, where the player stands as it
/// starts), or stops for everyone.
fn jukebox(game: &mut Game<'_>, uuid: u128, source: Source, dimension: &str, window: i32, i: usize) {
    let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
        return;
    };
    let Some((conn, position)) =
        game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| (g.conn, g.position))
    else {
        return;
    };
    let Some((mut items, carried)) =
        game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
    else {
        return;
    };
    if items.len() < tier.menu_slots() || i >= tier.upgrades {
        return;
    }
    let disc = items[tier.slots() + i * tb::TAB_SLOTS].clone();
    let Some(upgrade) = items[tier.storage + i].as_mut().filter(|u| tb::Tab::of(Some(&u.item)) == tb::Tab::Jukebox) else {
        return;
    };
    match playing(upgrade) {
        Some(sound) => {
            set_playing(upgrade, None);
            if let Some(host) = game.lan.as_mut() {
                horizon_client::lan::containers::travelersbackpack_menu::stop_sound(&mut host.hub, &sound);
            }
        }
        None => {
            let Some(sound) = disc.as_ref().and_then(disc_sound) else {
                return;
            };
            // `SoundSource.RECORDS`, as loud as a jukebox, for each player here:
            // a placed backpack's at it; one carried or worn follows its player
            // (`JukeboxWidget.MovingSound`).
            let here: Vec<u128> = game
                .lan
                .as_ref()
                .map(|h| h.hub.guests.iter().filter(|g| g.placed && g.dimension == dimension).map(|g| g.uuid).collect())
                .unwrap_or_default();
            let carrier = match source {
                Source::Other(owner) => owner,
                _ => uuid,
            };
            let entity = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == carrier)).map(|g| g.entity_id);
            match (source, entity) {
                (Source::Block(pos), _) => {
                    let at = if game.dim.id() == dimension { pos.map(|c| f64::from(c) + 0.5) } else { position };
                    for player in here {
                        game.native_play_sound_to(player, &sound, "record", Some(at), 4.0, 1.0);
                    }
                }
                (_, Some(entity)) => {
                    let message = format!("{entity}\t{sound}");
                    for player in here {
                        let _ = game.native_net_send_to_player(player, JUKEBOX_CHANNEL, message.as_bytes());
                    }
                }
                (_, None) => {}
            }
            set_playing(upgrade, Some(&sound));
        }
    }
    with(|s| s.extras.insert(uuid, (tier, items[tier.storage..].to_vec())));
    let data = full_data(game, uuid, &items, tier);
    if let Some(host) = game.lan.as_mut() {
        horizon_client::lan::containers::travelersbackpack_menu::set_items(&mut host.hub, conn, items, carried);
        horizon_client::lan::containers::travelersbackpack_menu::send_data(&mut host.hub, conn, window, &data);
    }
}

/// A placed backpack with the lantern upgrade lights its place
/// (`LanternUpgrade`, light 15): a light block over it, taken away with
/// the upgrade or the backpack.
const LANTERN_KEY: &str = "Lantern";

fn lantern_light(game: &mut Game<'_>, pos: [i32; 3], entity: &mut horizon_client::containers::ModBlockEntity, on: bool) {
    let above = [pos[0], pos[1] + 1, pos[2]];
    let lit = matches!(entity.nbt.get(&horizon_world::nbt::JavaString::from(LANTERN_KEY)), Some(Tag::Byte(1)));
    if on == lit {
        return;
    }
    let dimension = game.dim.id().to_owned();
    let here = game.native_block(above);
    if on {
        if here.ends_with("air")
            && let Some(light) = game.native_default_state("minecraft:light")
            && game.native_set_block(&dimension, above, light, 3)
        {
            entity.nbt.insert(LANTERN_KEY.into(), Tag::Byte(1));
        }
    } else {
        if here == "minecraft:light"
            && let Some(air) = game.native_default_state("minecraft:air")
        {
            game.native_set_block(&dimension, above, air, 3);
        }
        entity.nbt.remove(&horizon_world::nbt::JavaString::from(LANTERN_KEY));
    }
}

/// A placed backpack goes: its lantern's light goes too.
fn unlight(game: &mut Game<'_>, dimension: &str, pos: [i32; 3]) {
    let lit = game
        .native_mod_block_entity(pos)
        .is_some_and(|m| matches!(m.nbt.get(&horizon_world::nbt::JavaString::from(LANTERN_KEY)), Some(Tag::Byte(1))));
    let above = [pos[0], pos[1] + 1, pos[2]];
    if lit
        && game.native_block(above) == "minecraft:light"
        && let Some(air) = game.native_default_state("minecraft:air")
    {
        game.native_set_block(dimension, above, air, 3);
    }
}

/// The tab each upgrade slot shows with its slots: a closed one shows none.
fn shown_tabs(items: &[Option<ItemStack>], tier: tb::Tier) -> Vec<tb::Tab> {
    (0..tier.upgrades)
        .map(|i| match items.get(tier.storage + i).and_then(Option::as_ref) {
            Some(s) if tab_open(s) => tb::Tab::of(Some(&s.item)),
            _ => tb::Tab::Plain,
        })
        .collect()
}

/// The screen's extra data: each upgrade slot's tab, its top bit set when open.
fn layout_bytes(items: &[Option<ItemStack>], tier: tb::Tier) -> Vec<u8> {
    (0..tier.upgrades)
        .map(|i| {
            let stack = items.get(tier.storage + i).and_then(Option::as_ref);
            let tab = tb::Tab::of(stack.map(|s| &*s.item));
            tab.code() | if stack.is_some_and(tab_open) { 0x80 } else { 0 } | if stack.is_some_and(tag_mode) { 0x40 } else { 0 }
        })
        .collect()
}

fn tabs_of(items: &[Option<ItemStack>], tier: tb::Tier) -> Vec<tb::Tab> {
    (0..tier.upgrades).map(|i| tb::Tab::of(items.get(tier.storage + i).and_then(Option::as_ref).map(|s| &*s.item))).collect()
}

/// Where an upgrade keeps its tab's items (`BACKPACK_CONTAINER`): its
/// `minecraft:custom_data`, under this key, with its furnace's state.
const TAB_ITEMS_KEY: &str = "TravelersBackpackSlots";
const SMELTING_KEY: &str = "TravelersBackpackSmelting";

fn custom_data(stack: &ItemStack) -> Compound {
    match stack.data().and_then(|d| d.other("minecraft:custom_data")) {
        Some(Tag::Compound(data)) => data.clone(),
        _ => Compound::new(),
    }
}

fn set_custom_data(stack: &mut ItemStack, data: Compound) {
    let d = stack.data_mut();
    d.set_other("minecraft:custom_data", (!data.is_empty()).then_some(Tag::Compound(data)));
    d.wire = Default::default();
    stack.tidy();
}

/// The items upgrade `stack`'s tab keeps.
fn tab_items(stack: &ItemStack) -> Vec<Option<ItemStack>> {
    horizon_items::item_nbt::decode_items(custom_data(stack).get(&horizon_world::nbt::JavaString::from(TAB_ITEMS_KEY)), tb::TAB_SLOTS)
}

fn set_tab_items(stack: &mut ItemStack, slots: &[Option<ItemStack>]) {
    let mut data = custom_data(stack);
    if slots.iter().all(Option::is_none) {
        data.remove(&horizon_world::nbt::JavaString::from(TAB_ITEMS_KEY));
    } else {
        data.insert(TAB_ITEMS_KEY.into(), horizon_items::item_nbt::encode_items(slots));
    }
    set_custom_data(stack, data);
}

/// Each tab that keeps its items has them written into its upgrade (a
/// crafting grid without its result).
fn keep_tab_items(items: &mut [Option<ItemStack>], tier: tb::Tier) {
    for (i, tab) in shown_tabs(items, tier).into_iter().enumerate() {
        if !tab.keeps_items() {
            continue;
        }
        let base = tier.slots() + i * tb::TAB_SLOTS;
        let mut kept: Vec<Option<ItemStack>> = items[base..base + tb::TAB_SLOTS].to_vec();
        if tab == tb::Tab::Crafting {
            kept[9] = None;
        }
        if let Some(upgrade) = items[tier.storage + i].as_mut() {
            set_tab_items(upgrade, &kept);
        }
    }
}

/// Upgrade slot `i`'s tab block emptied (its upgrade is taken out): what a
/// tank's slots hold goes back to the player; the rest is in the upgrade.
fn empty_tab_block(game: &mut Game<'_>, uuid: u128, items: &mut [Option<ItemStack>], tier: tb::Tier, i: usize, tab: tb::Tab) {
    let base = tier.slots() + i * tb::TAB_SLOTS;
    for k in base..base + tb::TAB_SLOTS {
        if let Some(stack) = items[k].take()
            && tab == tb::Tab::Tanks
        {
            give_back(game, uuid, stack);
        }
    }
}

/// A menu opens: each tab's block holds what its upgrade kept.
fn load_tab_items(game: &Game<'_>, items: &mut Vec<Option<ItemStack>>, tier: tb::Tier) {
    items.resize(tier.menu_slots(), None);
    for (i, tab) in shown_tabs(items, tier).into_iter().enumerate() {
        if !tab.keeps_items() {
            continue;
        }
        let Some(kept) = items[tier.storage + i].as_ref().map(tab_items) else {
            continue;
        };
        let base = tier.slots() + i * tb::TAB_SLOTS;
        for (k, stack) in kept.into_iter().enumerate().take(tab.slots().len()) {
            items[base + k] = stack;
        }
    }
    work_crafting(game, items, tier);
}

/// The tabs at work on a click: the tanks pour, the crafting grid shows
/// its result.
fn work_tabs(game: &Game<'_>, items: &mut [Option<ItemStack>], tier: tb::Tier) {
    work_tanks(items, tier);
    work_crafting(game, items, tier);
}

// ---- the crafting upgrade (`CraftingUpgrade`) -------------------------------------------------

/// Each crafting tab's result slot shows what its grid makes.
fn work_crafting(game: &Game<'_>, items: &mut [Option<ItemStack>], tier: tb::Tier) {
    for (i, tab) in shown_tabs(items, tier).into_iter().enumerate() {
        if tab != tb::Tab::Crafting {
            continue;
        }
        let base = tier.slots() + i * tb::TAB_SLOTS;
        let grid: Vec<Option<ItemStack>> = items[base..base + 9].to_vec();
        items[base + 9] = game.recipes().craft(&grid, 3);
    }
}

/// `ResultSlot.onTake`: the result to the cursor (stacked onto what it
/// holds), one of each ingredient used, what they leave (a bucket) back in
/// the grid; with shift, as many as the grid makes into the inventory.
fn take_crafted(
    game: &mut Game<'_>,
    uuid: u128,
    items: &mut [Option<ItemStack>],
    carried: &mut Option<ItemStack>,
    base: usize,
    shift: bool,
) {
    for _ in 0..if shift { 64 } else { 1 } {
        let grid: Vec<Option<ItemStack>> = items[base..base + 9].to_vec();
        let Some((id, made)) = game.recipes().find(&grid, 3).map(|(id, made)| (id.to_owned(), made)) else {
            return;
        };
        if shift {
            if shift_out(game, uuid, Some(made.clone())).is_some() {
                return;
            }
        } else {
            let max = game.items().get(&made.item).map_or(64, |i| i.max_stack.max(1));
            match carried.as_mut() {
                None => *carried = Some(made.clone()),
                Some(held) if same_item(held, &made) && held.count + made.count <= max => held.count += made.count,
                Some(_) => return,
            }
        }
        let rests = game.recipes().remainders(&id, &grid, 3);
        for (k, rest) in rests.into_iter().enumerate() {
            let cell = &mut items[base + k];
            if let Some(stack) = cell.as_mut() {
                stack.count -= 1;
                if stack.count == 0 {
                    *cell = None;
                }
            }
            if let Some(rest) = rest {
                if cell.is_none() {
                    *cell = Some(rest);
                } else {
                    give_back(game, uuid, rest);
                }
            }
        }
        let grid: Vec<Option<ItemStack>> = items[base..base + 9].to_vec();
        items[base + 9] = game.recipes().craft(&grid, 3);
    }
}

// ---- the smelting upgrades (`AbstractSmeltingUpgrade`) ----------------------------------------

/// A fuel's burn time in ticks (`AbstractFurnaceBlockEntity.getBurnDuration`),
/// a smoker's and a blast furnace's (`fast`) halved by the fuel's providers.
fn fuel_ticks_for(game: &Game<'_>, stack: &ItemStack, fast: bool) -> i32 {
    game.res.providers().burn_time(game.items(), stack, fast)
}

fn fuel_ticks(game: &Game<'_>, stack: &ItemStack) -> i32 {
    fuel_ticks_for(game, stack, false)
}

/// A furnace tab's state: cooking progress and total, burn left and total.
type Smelting = [i32; 4];

fn smelting_of(stack: &ItemStack) -> Smelting {
    match custom_data(stack).get(&horizon_world::nbt::JavaString::from(SMELTING_KEY)) {
        Some(Tag::IntArray(v)) if v.len() == 4 => [v[0], v[1], v[2], v[3]],
        _ => [0; 4],
    }
}

fn set_smelting(stack: &mut ItemStack, state: Smelting) {
    let mut data = custom_data(stack);
    if state == [0; 4] {
        data.remove(&horizon_world::nbt::JavaString::from(SMELTING_KEY));
    } else {
        data.insert(SMELTING_KEY.into(), Tag::IntArray(state.to_vec()));
    }
    set_custom_data(stack, data);
}

/// One tick of a furnace tab (`AbstractFurnaceBlockEntity.serverTick`): it
/// burns fuel while it has something to cook, cooks the input into the
/// output, cools when it can't. A smoker and a blast furnace cook twice as
/// fast and burn fuel twice as fast. Whether anything changed.
fn smelt(game: &Game<'_>, tab: tb::Tab, state: &mut Smelting, slots: &mut [Option<ItemStack>]) -> bool {
    let Some(kind) = tab.cooking() else {
        return false;
    };
    let before = (*state, slots.to_vec());
    let [cook, total, burn, burn_total] = state;
    if *burn > 0 {
        *burn -= 1;
    }
    let recipe = slots[0].as_ref().and_then(|input| game.recipes().cook(kind, input)).map(|c| (c.result.clone(), c.time as i32));
    let room = |result: &ItemStack, out: &Option<ItemStack>| match out {
        None => true,
        Some(out) => {
            same_item(out, result) && out.count + result.count <= game.items().get(&out.item).map_or(64, |i| i.max_stack)
        }
    };
    let can_cook = recipe.as_ref().is_some_and(|(result, _)| room(result, &slots[2]));
    if *burn == 0 && can_cook {
        if let Some(fuel) = slots[1].as_mut() {
            let ticks = fuel_ticks_for(game, fuel, tab != tb::Tab::Furnace);
            if ticks > 0 {
                *burn = ticks;
                *burn_total = ticks;
                let rest = (&*fuel.item == "minecraft:lava_bucket").then(|| ItemStack::new("minecraft:bucket", 1));
                fuel.count -= 1;
                if fuel.count == 0 {
                    slots[1] = rest;
                }
            }
        }
    }
    match recipe {
        Some((result, time)) if *burn > 0 && can_cook => {
            *total = time.max(1);
            *cook += 1;
            if *cook >= *total {
                *cook = 0;
                match slots[2].as_mut() {
                    Some(out) => out.count += result.count,
                    None => slots[2] = Some(result),
                }
                if let Some(input) = slots[0].as_mut() {
                    input.count -= 1;
                    if input.count == 0 {
                        slots[0] = None;
                    }
                }
            }
        }
        // Lit (or about to be) but nothing to cook: the progress is lost; out,
        // it cools two a tick.
        _ if *burn > 0 || (slots[0].is_some() && slots[1].is_some()) => *cook = 0,
        _ => *cook = (*cook - 2).max(0),
    }
    if *burn == 0 {
        *burn_total = 0;
    }
    before != (*state, slots.to_vec())
}

/// Every furnace tab of a backpack's upgrade slots, its items in its upgrade
/// (a closed backpack): one tick. Whether anything changed.
fn smelt_kept(game: &Game<'_>, items: &mut [Option<ItemStack>], tier: tb::Tier) -> bool {
    let mut changed = false;
    for (i, tab) in tabs_of(items, tier).into_iter().enumerate() {
        if tab.cooking().is_none() {
            continue;
        }
        let Some(upgrade) = items[tier.storage + i].as_mut().filter(|u| upgrade_enabled(u)) else {
            continue;
        };
        let mut slots = tab_items(upgrade);
        let mut state = smelting_of(upgrade);
        if smelt(game, tab, &mut state, &mut slots[..3]) {
            set_tab_items(upgrade, &slots);
            set_smelting(upgrade, state);
            changed = true;
        }
    }
    changed
}

/// What an open menu shows goes back into its backpack as it changes, not
/// only when it closes: a world closed (or a player leaving) with the menu
/// open keeps what was put in.
fn sync_open_menus(game: &mut Game<'_>) {
    let open: Vec<u128> = with(|s| s.open.keys().copied().filter(|u| !s.settings.contains_key(u)).collect());
    for uuid in open {
        sync_menu(game, uuid);
    }
}

/// What `uuid`'s open menu holds, into its backpack, when that backpack is
/// still the one it opened on (`menu_intact`); otherwise the menu closes
/// with nothing written, so nothing is duplicated.
fn sync_menu(game: &mut Game<'_>, uuid: u128) {
    let Some((source, dimension, _)) = with(|s| s.open.get(&uuid).cloned()) else {
        return;
    };
    if with(|s| s.settings.contains_key(&uuid)) {
        return;
    }
    let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
        return;
    };
    let here = game.dim.id().to_owned();
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    // A placed backpack's menu holds only near it, in its dimension.
    if let Source::Block(pos) = source {
        // Written and checked in its own level's pass.
        if dimension != here {
            return;
        }
        // Out of reach (8 blocks; in another level), unless an operator opened it
        // with `/tb access`: what it holds goes back, and it closes.
        let near = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).is_some_and(|g| {
            g.dimension == dimension && pos.iter().zip(g.position).all(|(b, p)| (f64::from(*b) + 0.5 - p).abs() <= 8.5)
        });
        if !near && !with(|s| s.access.contains(&uuid)) {
            if menu_intact(game, uuid, source, &dimension) {
                write_menu(game, uuid, conn, source, &dimension, tier);
            }
            close_now(game, uuid);
            return;
        }
    }
    if !menu_intact(game, uuid, source, &dimension) {
        close_now(game, uuid);
        return;
    }
    write_menu(game, uuid, conn, source, &dimension, tier);
}

/// `uuid`'s menu as it is now, into its backpack.
fn write_menu(game: &mut Game<'_>, uuid: u128, conn: u32, source: Source, dimension: &str, tier: tb::Tier) {
    let Some((mut items, _)) =
        game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
    else {
        return;
    };
    if items.len() < tier.menu_slots() {
        return;
    }
    keep_tab_items(&mut items, tier);
    items.truncate(tier.slots());
    if with(|s| s.synced.get(&uuid) == Some(&items)) {
        return;
    }
    match source {
        Source::Hand(slot) => {
            let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)) else {
                return;
            };
            if let Some(stack) = g.inventory.get_mut(slot).and_then(Option::as_mut)
                && tb::is_backpack(&stack.item)
            {
                set_contents(stack, items.clone());
            }
        }
        Source::Worn => {
            if let Some(mut stack) = worn(game, uuid) {
                set_contents(&mut stack, items.clone());
                set_worn(game, uuid, Some(&stack));
            }
        }
        Source::Other(owner) => {
            if let Some(mut stack) = worn(game, owner) {
                set_contents(&mut stack, items.clone());
                set_worn(game, owner, Some(&stack));
            }
        }
        Source::Block(pos) => {
            if game.dim.id() != dimension {
                return;
            }
            let copy = items.clone();
            game.native_with_mod_block_entity(pos, |m| {
                if m.id != tb::BLOCK_ENTITY {
                    return;
                }
                for (i, item) in copy.into_iter().enumerate() {
                    if let Some(slot) = m.items.get_mut(i) {
                        *slot = item;
                    }
                }
            });
        }
    }
    with(|s| s.synced.insert(uuid, items));
}

/// The key a backpack holds while a menu is open on it: which opening.
const OPEN_TOKEN_KEY: &str = "TravelersBackpackOpen";

thread_local! {
    /// Each open menu's token, the one on its backpack.
    static TOKENS: RefCell<HashMap<u128, i64>> = RefCell::new(HashMap::new());
    static NEXT_TOKEN: std::cell::Cell<i64> = const { std::cell::Cell::new(1) };
}

/// The menu just opened marks its backpack (an item in the hand, or a placed one).
fn mark_open(game: &mut Game<'_>, uuid: u128, source: Source) {
    let token = NEXT_TOKEN.with(|n| {
        let t = n.get();
        n.set(t.wrapping_add(1));
        t ^ (game.world.game_time << 20)
    });
    TOKENS.with(|t| t.borrow_mut().insert(uuid, token));
    match source {
        Source::Hand(slot) => {
            if let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid))
                && let Some(stack) = g.inventory.get_mut(slot).and_then(Option::as_mut)
            {
                let mut data = custom_data(stack);
                data.insert(OPEN_TOKEN_KEY.into(), Tag::Long(token));
                set_custom_data(stack, data);
            }
        }
        Source::Block(pos) => {
            game.native_with_mod_block_entity(pos, |m| {
                m.nbt.insert(OPEN_TOKEN_KEY.into(), Tag::Long(token));
            });
        }
        Source::Worn | Source::Other(_) => {}
    }
}

/// Whether `uuid`'s open menu's backpack is still where it was opened, the
/// same one: the item in that slot, or the placed backpack, with its token.
fn menu_intact(game: &Game<'_>, uuid: u128, source: Source, dimension: &str) -> bool {
    let Some(token) = TOKENS.with(|t| t.borrow().get(&uuid).copied()) else {
        return !matches!(source, Source::Hand(_) | Source::Block(_));
    };
    let key = horizon_world::nbt::JavaString::from(OPEN_TOKEN_KEY);
    match source {
        Source::Hand(slot) => game
            .lan
            .as_ref()
            .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid))
            .and_then(|g| g.inventory.get(slot).cloned().flatten())
            .is_some_and(|s| tb::is_backpack(&s.item) && custom_data(&s).get(&key) == Some(&Tag::Long(token))),
        Source::Block(pos) => {
            game.dim.id() == dimension
                && tb::is_backpack(&game.native_block(pos))
                && game.native_mod_block_entity(pos).is_some_and(|m| m.id == tb::BLOCK_ENTITY && m.nbt.get(&key) == Some(&Tag::Long(token)))
        }
        Source::Worn => worn(game, uuid).is_some(),
        Source::Other(owner) => worn(game, owner).is_some(),
    }
}

/// The open menu's token off its backpack (the menu closed).
fn unmark_open(game: &mut Game<'_>, uuid: u128, source: Source) {
    TOKENS.with(|t| t.borrow_mut().remove(&uuid));
    let key = horizon_world::nbt::JavaString::from(OPEN_TOKEN_KEY);
    match source {
        Source::Hand(slot) => {
            if let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid))
                && let Some(stack) = g.inventory.get_mut(slot).and_then(Option::as_mut)
            {
                let mut data = custom_data(stack);
                if data.remove(&key).is_some() {
                    set_custom_data(stack, data);
                }
            }
        }
        Source::Block(pos) => {
            if tb::is_backpack(&game.native_block(pos)) {
                game.native_with_mod_block_entity(pos, |m| {
                    m.nbt.remove(&key);
                });
            }
        }
        Source::Worn | Source::Other(_) => {}
    }
}

/// `uuid`'s menu closes now, nothing written back (its backpack is gone or another).
fn close_now(game: &mut Game<'_>, uuid: u128) {
    let Some((source, _, window)) = with(|s| s.open.remove(&uuid)) else {
        return;
    };
    let tier = with(|s| s.extras.get(&uuid).map(|e| e.0));
    // A tank tab's buckets and bottles are the player's: they go back.
    let conn = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn);
    if let (Some(tier), Some(conn)) = (tier, conn)
        && let Some((items, _)) = game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
        && items.len() >= tier.menu_slots()
    {
        let tabs = shown_tabs(&items, tier);
        let back: Vec<ItemStack> = items[tier.slots()..]
            .iter()
            .enumerate()
            .filter(|(n, _)| tabs.get(n / tb::TAB_SLOTS) == Some(&tb::Tab::Tanks))
            .filter_map(|(_, s)| s.clone())
            .collect();
        for stack in back {
            give_back(game, uuid, stack);
        }
    }
    with(|s| {
        s.synced.remove(&uuid);
        s.extras.remove(&uuid);
        s.settings.remove(&uuid);
        s.access.remove(&uuid);
    });
    unmark_open(game, uuid, source);
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    if let Some(host) = game.lan.as_mut() {
        // The menu's slots are a copy of the backpack: emptied first, or
        // closing would hand them to the player (`closeContainer`).
        use horizon_client::lan::containers::travelersbackpack_menu as menu;
        let open = host.hub.guests.iter().find(|g| g.conn == conn).is_some_and(|g| g.menu.as_ref().is_some_and(|m| m.window == window));
        if open && let Some((items, carried)) = menu::items(&host.hub, conn) {
            menu::set_items(&mut host.hub, conn, vec![None; items.len()], carried);
        }
        menu::close(&mut host.hub, conn, window);
    }
}

/// The menus open on the placed backpack at `pos`: what they hold goes into
/// it first, then they close (it is about to be taken, broken or changed).
fn flush_block_menus(game: &mut Game<'_>, pos: [i32; 3]) {
    let viewers: Vec<u128> = with(|s| s.open.iter().filter(|(_, o)| o.0 == Source::Block(pos)).map(|(u, _)| *u).collect());
    for uuid in viewers {
        sync_menu(game, uuid);
        close_now(game, uuid);
    }
}

/// The furnace tabs of the open menus: one tick, on what the menus show.
fn smelt_open_menus(game: &mut Game<'_>) {
    let open: Vec<(u128, i32, tb::Tier)> = with(|s| {
        s.open
            .iter()
            .filter(|(u, _)| !s.settings.contains_key(u))
            .filter_map(|(u, o)| s.extras.get(u).map(|e| (*u, o.2, e.0)))
            .collect()
    });
    let dimension = game.dim.id().to_owned();
    for (uuid, window, tier) in open {
        // Once a tick: the server tick is heard in each dimension.
        let Some(conn) = game
            .lan
            .as_ref()
            .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid && g.dimension == dimension))
            .map(|g| g.conn)
        else {
            continue;
        };
        let Some((mut items, carried)) =
            game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
        else {
            continue;
        };
        if items.len() < tier.menu_slots() {
            continue;
        }
        let mut changed = false;
        let mut cooking = false;
        let shown = shown_tabs(&items, tier);
        for (i, tab) in tabs_of(&items, tier).into_iter().enumerate() {
            if tab.cooking().is_none() || items[tier.storage + i].as_ref().is_some_and(|u| !upgrade_enabled(u)) {
                continue;
            }
            cooking = true;
            let base = tier.slots() + i * tb::TAB_SLOTS;
            let Some(mut upgrade) = items[tier.storage + i].clone() else {
                continue;
            };
            let mut state = smelting_of(&upgrade);
            if shown[i] == tab {
                // Open: on what its slots show.
                if smelt(game, tab, &mut state, &mut items[base..base + 3]) {
                    set_smelting(&mut upgrade, state);
                    items[tier.storage + i] = Some(upgrade);
                    changed = true;
                }
            } else {
                // Closed: on what its upgrade keeps.
                let mut kept = tab_items(&upgrade);
                if smelt(game, tab, &mut state, &mut kept[..3]) {
                    set_tab_items(&mut upgrade, &kept);
                    set_smelting(&mut upgrade, state);
                    items[tier.storage + i] = Some(upgrade);
                    changed = true;
                }
            }
        }
        if !cooking {
            continue;
        }
        let data = full_data(game, uuid, &items, tier);
        with(|s| {
            if let Some(e) = s.extras.get_mut(&uuid) {
                e.1 = items[tier.storage..].to_vec();
            }
        });
        if let Some(host) = game.lan.as_mut() {
            if changed {
                horizon_client::lan::containers::travelersbackpack_menu::set_items(&mut host.hub, conn, items, carried);
            }
            horizon_client::lan::containers::travelersbackpack_menu::send_data(&mut host.hub, conn, window, &data);
        }
    }
}

// ---- the filter upgrades (`VoidUpgrade`, `RefillUpgrade`) -------------------------------------

/// The items a backpack's tab of `tab` filters for.
fn filter_of(items: &[Option<ItemStack>], tier: tb::Tier, tab: tb::Tab) -> Vec<ItemStack> {
    tabs_of(items, tier)
        .into_iter()
        .enumerate()
        .filter(|&(_, t)| t == tab)
        .filter_map(|(i, _)| items[tier.storage + i].as_ref())
        .filter(|u| upgrade_enabled(u))
        .flat_map(|u| tab_items(u).into_iter().take(9).flatten())
        .collect()
}

/// `RefillUpgrade.REFILL_RANGE`.
const REFILL_RANGE: f64 = 3.0;

/// `RefillUpgrade.refill`: each filtered item the player carries less than a
/// stack of is topped up from the storage (into its stack, else a free slot).
/// A placed backpack refills the players within 3 blocks of it (`REFILL_RANGE`).
fn refill(game: &mut Game<'_>, uuid: u128, items: &mut [Option<ItemStack>], tier: tb::Tier) -> bool {
    let wanted = filter_of(items, tier, tb::Tab::Refill);
    let mut changed = false;
    for want in wanted {
        let max = game.items().get(&want.item).map_or(64, |i| i.max_stack.max(1));
        let Some(g) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)) else {
            return changed;
        };
        // The stack on their cursor counts too (`countAndSupply`).
        let held: u32 = g.inventory.iter().take(36).chain(std::iter::once(&g.carried)).flatten().filter(|s| same_item(s, &want)).map(|s| s.count).sum();
        if held >= max {
            continue;
        }
        let target = g
            .inventory
            .iter()
            .take(36)
            .position(|s| s.as_ref().is_some_and(|s| same_item(s, &want) && s.count < max))
            .or_else(|| free_slot(&g.inventory));
        let Some(target) = target else {
            continue;
        };
        let mut missing = max - held;
        let mut got = 0;
        for slot in items[..tier.storage].iter_mut() {
            if missing == 0 {
                break;
            }
            if let Some(stack) = slot.as_mut()
                && same_item(stack, &want)
            {
                let n = stack.count.min(missing);
                stack.count -= n;
                missing -= n;
                got += n;
                if stack.count == 0 {
                    *slot = None;
                }
            }
        }
        if got == 0 {
            continue;
        }
        changed = true;
        if let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)) {
            match g.inventory[target].as_mut() {
                Some(stack) => stack.count += got,
                None => {
                    let mut stack = want.clone();
                    stack.count = got;
                    g.inventory[target] = Some(stack);
                }
            }
        }
    }
    changed
}

/// The menu's data slots, five for each upgrade slot: the tanks' fluids,
/// amounts and capacity; a furnace's cooking progress and total, burn left
/// and total.
fn menu_data(items: &[Option<ItemStack>], tier: tb::Tier) -> Vec<i32> {
    let mut data = vec![0; 5 * tier.upgrades];
    for (i, tab) in tabs_of(items, tier).into_iter().enumerate() {
        let Some(upgrade) = items[tier.storage + i].as_ref() else {
            continue;
        };
        let values: [i32; 5] = match tab {
            tb::Tab::Tanks => {
                let t = tanks_of(upgrade);
                [fluid_code(&t[0].0), t[0].1, fluid_code(&t[1].0), t[1].1, tank_capacity(tier)]
            }
            t if t.cooking().is_some() => {
                let s = smelting_of(upgrade);
                [s[0], s[1], s[2], s[3], 0]
            }
            t if t.filter_states().is_some() => {
                let f = filter_settings(upgrade);
                [f[0], f[1], f[2], 0, 0]
            }
            _ => [0; 5],
        };
        data[5 * i..5 * i + 5].copy_from_slice(&values);
    }
    data
}

// ---- the tanks (`TanksUpgrade`) ------------------------------------------------------------

const TANKS_UPGRADE: &str = "travelersbackpack:tanks_upgrade";
/// The fluids a tank holds, by the number the screen's data and the model
/// get (0 is none): each with its bucket and the colour drawn for it. A
/// potion of any kind is `potion:<its id>` (`PotionFluid`), drawn in one colour.
const FLUIDS: [(&str, &str, u32); 4] = [
    ("minecraft:water", "minecraft:water_bucket", 0xFF3F_76E4),
    ("minecraft:lava", "minecraft:lava_bucket", 0xFFD9_6415),
    ("minecraft:milk", "minecraft:milk_bucket", 0xFFF2_F2F2),
    ("potion", "", 0xFFB0_3FD0),
];
/// A bucket and a bottle (`FluidConstants.BUCKET`, `BOTTLE`), in thirds of a
/// millibucket: a bottle is a third of a bucket, as on Fabric (27000 droplets
/// of 81000). Shown in mB (`FLUID_UNIT` a millibucket).
const BUCKET: i32 = 3000;
const BOTTLE: i32 = 1000;
const FLUID_UNIT: i32 = 3;
/// Tanks saved before thirds (in mB) have no unit; they are read times three.
const FLUID_UNIT_KEY: &str = "Unit";

/// An amount in mB, to show.
fn mb(amount: i32) -> i32 {
    amount / FLUID_UNIT
}
/// Where the tanks upgrade keeps its fluids (`ModDataComponents.FLUIDS`): its
/// `minecraft:custom_data`, under this key.
const FLUIDS_KEY: &str = "TravelersBackpackFluids";

/// The number of fluid `fluid` (`""` none).
fn fluid_code(fluid: &str) -> i32 {
    if potion_fluid(fluid).is_some() {
        return 4;
    }
    FLUIDS.iter().position(|f| f.0 == fluid).map_or(0, |i| i as i32 + 1)
}

/// The potion a potion item holds (`DataComponents.POTION_CONTENTS`).
fn potion_of(stack: &ItemStack) -> Option<String> {
    stack.potion().map(str::to_owned)
}

/// A potion item of `potion`.
fn potion_item(potion: &str) -> ItemStack {
    ItemStack::new("minecraft:potion", 1).with_potion(potion)
}

/// The kinds of potion a tank keeps apart (`PotionFluid` and its
/// `PotionType`: drunk, splash, lingering): `<kind>:<potion id>`.
const POTION_KINDS: [&str; 3] = ["potion", "splash_potion", "lingering_potion"];

/// A potion fluid's bottle (`minecraft:splash_potion`) and potion.
fn potion_fluid(fluid: &str) -> Option<(&'static str, &str)> {
    POTION_KINDS.iter().find_map(|kind| {
        let id = fluid.strip_prefix(kind)?.strip_prefix(':')?;
        Some((match *kind {
            "splash_potion" => "minecraft:splash_potion",
            "lingering_potion" => "minecraft:lingering_potion",
            _ => "minecraft:potion",
        }, id))
    })
}

/// A fluid's colour, RGB (`FluidVariantRendering.getColor`): a potion's its
/// effects' mix (`PotionContents.getColor`), the others their own.
fn fluid_colour(fluid: &str) -> u32 {
    match potion_fluid(fluid) {
        Some((_, id)) => horizon_client::brewing::potion_color(Some(id)),
        None => FLUIDS.iter().find(|f| f.0 == fluid).map_or(0xFFFFFF, |f| f.2 & 0xFF_FFFF),
    }
}

/// A tank's fluid for the models and the bar: its number, a potion's carrying
/// its colour (`4 + (rgb << 3)`).
fn tank_code(fluid: &str) -> i32 {
    match potion_fluid(fluid) {
        Some(_) => 4 + ((fluid_colour(fluid) as i32) << 3),
        None => fluid_code(fluid),
    }
}

/// The colour of a tank's fluid by its `tank_code`, RGB.
fn tank_code_colour(code: i32) -> u32 {
    match code {
        c if c > 4 => ((c - 4) >> 3) as u32,
        c => usize::try_from(c - 1).ok().and_then(|i| FLUIDS.get(i)).map_or(0xFFFFFF, |f| f.2 & 0xFF_FFFF),
    }
}

/// Each tank's fluid colour for the screen's data slots (16 bits a slot):
/// high and low halves, left then right.
fn tank_colour_words(items: &[Option<ItemStack>], tier: tb::Tier) -> [i32; 4] {
    let Some(upgrade) = tanks_slot(items, tier).and_then(|i| items[tier.storage + i].as_ref()) else {
        return [0; 4];
    };
    let [l, r] = tanks_of(upgrade).map(|(f, _)| fluid_colour(&f) as i32);
    [l >> 16, l & 0xFFFF, r >> 16, r & 0xFFFF]
}

/// What a fluid container does at a tank's input (`FluidStorage.ITEM`): a
/// full one pours its fluid and leaves its empty container; an empty one
/// (a bucket, a glass bottle) takes as much as it holds.
enum Container {
    Pour { fluid: String, amount: i32, empty: &'static str },
    Fill { amount: i32, bottle: bool },
}

fn container_of(stack: &ItemStack) -> Option<Container> {
    match &*stack.item {
        "minecraft:bucket" => Some(Container::Fill { amount: BUCKET, bottle: false }),
        "minecraft:glass_bottle" => Some(Container::Fill { amount: BOTTLE, bottle: true }),
        "minecraft:potion" => {
            let potion = potion_of(stack)?;
            let fluid = if potion == "minecraft:water" { "minecraft:water".to_owned() } else { format!("potion:{potion}") };
            Some(Container::Pour { fluid, amount: BOTTLE, empty: "minecraft:glass_bottle" })
        }
        "minecraft:splash_potion" | "minecraft:lingering_potion" => {
            let potion = potion_of(stack)?;
            let kind = stack.item.strip_prefix("minecraft:").unwrap_or_default();
            Some(Container::Pour { fluid: format!("{kind}:{potion}"), amount: BOTTLE, empty: "" })
        }
        item => FLUIDS.iter().find(|f| !f.1.is_empty() && f.1 == item).map(|f| Container::Pour {
            fluid: f.0.to_owned(),
            amount: BUCKET,
            empty: "minecraft:bucket",
        }),
    }
}

/// The container filled with `fluid`: a bucket of water, lava or milk; a
/// bottle of water or of a potion.
fn filled(fluid: &str, bottle: bool) -> Option<ItemStack> {
    if bottle {
        return match fluid {
            "minecraft:water" => Some(potion_item("minecraft:water")),
            f => potion_fluid(f).map(|(bottle, id)| ItemStack::new(bottle, 1).with_potion(id)),
        };
    }
    FLUIDS.iter().find(|f| f.0 == fluid && !f.1.is_empty()).map(|f| ItemStack::new(f.1, 1))
}

/// The two tanks: (fluid, amount in mB) each.
type Tanks = [(String, i32); 2];

/// What the tanks upgrade `stack` holds.
fn tanks_of(stack: &ItemStack) -> Tanks {
    let empty = || [(String::new(), 0), (String::new(), 0)];
    let Some(Tag::Compound(f)) = custom_data(stack).get(&horizon_world::nbt::JavaString::from(FLUIDS_KEY)).cloned() else {
        return empty();
    };
    let get = |k: &str| f.get(&horizon_world::nbt::JavaString::from(k)).cloned();
    let scale = if matches!(get(FLUID_UNIT_KEY), Some(Tag::Int(FLUID_UNIT))) { 1 } else { FLUID_UNIT };
    let tank = |fluid: &str, amount: &str| {
        let amount = match get(amount) {
            Some(Tag::Int(n)) => n.max(0).saturating_mul(scale),
            _ => 0,
        };
        let fluid = match get(fluid) {
            Some(Tag::String(s)) => s.to_string().unwrap_or_default(),
            // Kept as a number before potions came.
            Some(Tag::Int(n)) => usize::try_from(n - 1).ok().and_then(|i| FLUIDS.get(i)).map(|f| f.0.to_owned()).unwrap_or_default(),
            _ => String::new(),
        };
        if fluid.is_empty() || amount == 0 { (String::new(), 0) } else { (fluid, amount) }
    };
    [tank("LeftFluid", "Left"), tank("RightFluid", "Right")]
}

/// The tanks upgrade `stack` holds `tanks` from now on.
fn set_tanks(stack: &mut ItemStack, tanks: &Tanks) {
    let mut data = custom_data(stack);
    let mut f = Compound::new();
    for (side, (fluid, amount)) in ["Left", "Right"].into_iter().zip(tanks) {
        f.insert(format!("{side}Fluid").as_str().into(), Tag::String(fluid.as_str().into()));
        f.insert(side.into(), Tag::Int(*amount));
    }
    f.insert(FLUID_UNIT_KEY.into(), Tag::Int(FLUID_UNIT));
    data.insert(FLUIDS_KEY.into(), Tag::Compound(f));
    set_custom_data(stack, data);
}

/// The upgrade slot holding the tanks upgrade, if one does.
fn tanks_slot(items: &[Option<ItemStack>], tier: tb::Tier) -> Option<usize> {
    (0..tier.upgrades)
        .find(|&i| items.get(tier.storage + i).and_then(Option::as_ref).is_some_and(|s| &*s.item == TANKS_UPGRADE))
}

fn has_tanks_upgrade(items: &[Option<ItemStack>], tier: tb::Tier) -> bool {
    tanks_slot(items, tier).is_some()
}

/// A backpack's tanks: each one's fluid number and how full (0 to 1), for the model.
fn tank_levels(items: &[Option<ItemStack>], tier: tb::Tier) -> Option<[(i32, f32); 2]> {
    let i = tanks_slot(items, tier)?;
    let tanks = tanks_of(items[tier.storage + i].as_ref()?);
    let capacity = tank_capacity(tier).max(1) as f32;
    Some(tanks.map(|(f, a)| (tank_code(&f), (a as f32 / capacity).clamp(0.0, 1.0))))
}

/// `TankActions`: each tank's input slot pours its container in (the empty
/// one to its output), or an empty bucket or bottle there fills from the
/// tank, while the output has room.
fn work_tanks(items: &mut [Option<ItemStack>], tier: tb::Tier) {
    let Some(i) = tanks_slot(items, tier) else {
        return;
    };
    let k = tier.storage + i;
    let Some(upgrade) = items[k].clone() else {
        return;
    };
    let base = tier.slots() + i * tb::TAB_SLOTS;
    if items.len() < base + 4 {
        return;
    }
    let mut tanks = tanks_of(&upgrade);
    let capacity = tank_capacity(tier);
    for (side, tank) in tanks.iter_mut().enumerate() {
        let (input, output) = (base + 2 * side, base + 2 * side + 1);
        loop {
            let Some(held) = items[input].clone() else {
                break;
            };
            let Some(container) = container_of(&held) else {
                break;
            };
            let result = match container {
                Container::Fill { amount, bottle } => {
                    let Some(full) = filled(&tank.0, bottle).filter(|_| tank.1 >= amount) else {
                        break;
                    };
                    if items[output].is_some() {
                        break;
                    }
                    tank.1 -= amount;
                    if tank.1 == 0 {
                        tank.0.clear();
                    }
                    full
                }
                Container::Pour { fluid, amount, empty } => {
                    if (!tank.0.is_empty() && tank.0 != fluid) || tank.1 + amount > capacity {
                        break;
                    }
                    let max = if empty == "minecraft:bucket" { 16 } else { 64 };
                    match &items[output] {
                        None => {}
                        _ if empty.is_empty() => {}
                        Some(out) if &*out.item == empty && out.count < max => {}
                        Some(_) => break,
                    }
                    tank.0 = fluid;
                    tank.1 += amount;
                    ItemStack::new(empty, if empty.is_empty() { 0 } else { 1 })
                }
            };
            if result.count > 0 {
                match items[output].as_mut() {
                    Some(out) if same_item(out, &result) => out.count += 1,
                    _ => items[output] = Some(result),
                }
            }
            let mut rest = held;
            rest.count -= 1;
            items[input] = (rest.count > 0).then_some(rest);
        }
    }
    let mut upgrade = upgrade;
    set_tanks(&mut upgrade, &tanks);
    items[k] = Some(upgrade);
}

/// The worn backpack's tanks upgrade and placed ones show on the model: the
/// block's `tanks` state.
fn shows_tanks(items: &[Option<ItemStack>], tier: tb::Tier) -> bool {
    has_tanks_upgrade(items, tier)
}

/// A placed backpack at `pos` shows its tanks or not (`TravelersBackpackBlock`'s
/// model with tanks), keeping its block entity's contents and tier.
fn set_block_tanks(game: &mut Game<'_>, dimension: &str, pos: [i32; 3], tanks: bool) {
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let registry = game.pack.assets.registry;
    let Some(mut wanted) = registry.state(state).cloned() else {
        return;
    };
    wanted.properties.insert("tanks".into(), if tanks { "true" } else { "false" }.into());
    // Laid out, the bag isn't rolled on the backpack (`isSleepingBagDeployed`).
    let bag = game.native_mod_block_entity(pos).map_or(RED, |m| block_bag(&m.nbt));
    let bag = if bag_deployed(game, pos) { "none" } else { bag_name(bag) };
    wanted.properties.insert("bag".into(), bag.into());
    if wanted.properties.contains_key("dyed") {
        let dyed = game.native_mod_block_entity(pos).is_some_and(|m| block_dye(&m.nbt).is_some());
        wanted.properties.insert("dyed".into(), if dyed { "true" } else { "false" }.into());
    }
    let Ok(new) = registry.resolve(&wanted) else {
        return;
    };
    if new == state {
        return;
    }
    let mut kept = None;
    game.native_with_mod_block_entity(pos, |m| kept = Some((m.items.clone(), m.nbt.clone())));
    if game.native_set_block(dimension, pos, new, 3)
        && let Some((items, nbt)) = kept
    {
        game.native_with_mod_block_entity(pos, |m| {
            m.items = items;
            m.nbt = nbt;
        });
    }
}

// ---- the upgrades at work (`ITickableUpgrade`) ---------------------------------------------

/// `AutoPickupUpgrade`: every 5 ticks, items within a block of the backpack.
/// The magnet's, the feeding's and the refill's rates and the magnet's range
/// are the config's (`MagnetUpgradeSettings`: every 10 ticks within 5 blocks;
/// `FeedingUpgradeSettings`: every 100; `RefillUpgradeSettings`: every 5).
const PICKUP_RATE: u64 = 5;
/// `FeedingUpgrade`: every 10 ticks while still hungry, players within 3.
const FEEDING_HUNGRY_RATE: u64 = 10;
const FEEDING_RANGE: f64 = 3.0;

/// The upgrades in a backpack's slots (`storage..storage + upgrades`).
fn upgrades_in(items: &[Option<ItemStack>], tier: tb::Tier) -> Vec<String> {
    items
        .iter()
        .skip(tier.storage)
        .take(tier.upgrades)
        .flatten()
        .filter(|s| upgrade_enabled(s) && !upgrade_disabled(&s.item))
        .map(|s| s.item.to_string())
        .collect()
}

/// An upgrade the config turns off (`enableTanksUpgrade` and the like).
fn upgrade_disabled(item: &str) -> bool {
    item.strip_prefix("travelersbackpack:").is_some_and(|name| cfg().disabled_upgrades.contains(&name))
}

/// `BackpackSlotItemHandler.isItemValid`: no backpack in a backpack, nor a
/// shulker box (`allowShulkerBoxes`), nor one of `blacklistedItems`.
fn fits_storage(game: &Game<'_>, stack: &ItemStack) -> bool {
    !tb::is_backpack(&stack.item)
        && (cfg().allow_shulker_boxes || !stack.item.ends_with("shulker_box"))
        && !cfg().blacklisted_items.iter().any(|i| **i == *stack.item)
        && !game.items().item_in_tag("travelersbackpack:blacklisted_items", &stack.item)
}

/// `stack` as the four ints an entity's UUID is saved as.
fn uuid_ints(uuid: u128) -> [i32; 4] {
    let b = uuid.to_be_bytes();
    std::array::from_fn(|i| i32::from_be_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]))
}

/// Whether `a` and `b` are the same item with the same components
/// (`ItemStack.isSameItemSameComponents`), so they stack.
fn same_item(a: &ItemStack, b: &ItemStack) -> bool {
    a.item == b.item && a.damage == b.damage && a.data() == b.data()
}

/// `InventoryHelper.insertItemStacked`: `stack` into the storage slots,
/// onto stacks of its kind first, then into empty ones. What didn't fit.
fn insert_stacked(game: &Game<'_>, storage: &mut [Option<ItemStack>], mut stack: ItemStack) -> Option<ItemStack> {
    let max = game.items().get(&stack.item).map_or(64, |i| i.max_stack.max(1));
    for slot in storage.iter_mut().flatten() {
        if stack.count == 0 {
            break;
        }
        if same_item(slot, &stack) && slot.count < max {
            let moved = (max - slot.count).min(stack.count);
            slot.count += moved;
            stack.count -= moved;
        }
    }
    for slot in storage.iter_mut() {
        if stack.count == 0 {
            break;
        }
        if slot.is_none() {
            let mut put = stack.clone();
            put.count = stack.count.min(max);
            stack.count -= put.count;
            *slot = Some(put);
        }
    }
    (stack.count > 0).then_some(stack)
}

/// `MagnetUpgrade.teleportNearbyItems`: items near `at` come to `to`, ready to
/// be picked up; the wearer's own throws only after 4 seconds.
fn magnet(game: &mut Game<'_>, at: [f64; 3], to: [f64; 3], wearer: Option<u128>, items: &[Option<ItemStack>], tier: tb::Tier) {
    let own = wearer.map(uuid_ints);
    let allowed: Vec<bool> =
        game.dropped.iter().map(|d| filter_matches(game, items, tier, tb::Tab::Magnet, &d.stack) != Some(false)).collect();
    for (item, allowed) in game.dropped.iter_mut().zip(allowed) {
        if !allowed {
            continue;
        }
        let p = item.position;
        let near = (f64::from(p.x) - at[0]).abs() <= f64::from(cfg().magnet_pull_range)
            && (f64::from(p.y) - at[1]).abs() <= f64::from(cfg().magnet_pull_range)
            && (f64::from(p.z) - at[2]).abs() <= f64::from(cfg().magnet_pull_range);
        let thrown_by_wearer = own.is_some() && item.thrower == own && item.age <= 80;
        if near && item.stack.count > 0 && !thrown_by_wearer {
            (item.position.x, item.position.y, item.position.z) = (to[0], to[1], to[2]);
            item.previous = item.position;
            item.pickup_delay = 0;
            item.needs_sync = true;
        }
    }
}

/// `AutoPickupUpgrade.tryPickup`: items within a block of block `at` go into
/// the storage (the first `storage` slots of `items`). Whether any did.
/// With the void upgrade, what its filter names is destroyed instead (`VoidUpgrade`).
/// `touched`: a wearer's touch, whose pickup delay the canceled pickup set.
fn pickup(game: &mut Game<'_>, at: [i32; 3], items: &mut [Option<ItemStack>], tier: tb::Tier, touched: bool) -> bool {
    let storage = tier.storage;
    let (lo, hi) = (at.map(|c| c - 1), at.map(|c| c + 2));
    let mut dropped = std::mem::take(&mut game.dropped);
    let mut any = false;
    for item in &mut dropped {
        let p = [item.position.x, item.position.y, item.position.z];
        let inside = (0..3).all(|i| f64::from(p[i]) >= f64::from(lo[i]) && f64::from(p[i]) < f64::from(hi[i]));
        if !inside || (item.pickup_delay > 0 && !touched) || item.stack.count == 0 || !fits_storage(game, &item.stack) {
            continue;
        }
        if filter_matches(game, items, tier, tb::Tab::Pickup, &item.stack) == Some(false) {
            continue;
        }
        if voids(game, items, tier, &item.stack) {
            item.stack.count = 0;
            item.needs_sync = true;
            any = true;
            continue;
        }
        let before = item.stack.count;
        let left = insert_stacked(game, &mut items[..storage], item.stack.clone());
        let after = left.as_ref().map_or(0, |s| s.count);
        if after != before {
            any = true;
            item.stack.count = after;
            item.needs_sync = true;
        }
    }
    game.dropped = dropped;
    any
}

/// Food the feeding upgrade may give (`FeedingFilterSettings` as it starts:
/// any food, none with a harmful effect, eaten when half its nutrition or more
/// is needed): its nutrition and saturation.
fn feed_value(game: &Game<'_>, stack: &ItemStack, food_level: i32, filter: &(Vec<ItemStack>, [i32; 3])) -> Option<(i32, f32)> {
    const HARMFUL: [&str; 6] = [
        "minecraft:rotten_flesh",
        "minecraft:spider_eye",
        "minecraft:poisonous_potato",
        "minecraft:pufferfish",
        "minecraft:chicken",
        "minecraft:suspicious_stew",
    ];
    let (nutrition, saturation) = game.items().get(&stack.item)?.food?;
    let nutrition = i32::try_from(nutrition).ok()?;
    let (foods, settings) = filter;
    let listed = foods.iter().any(|f| same_item(f, &ItemStack { count: 1, ..stack.clone() }) || (f.item == stack.item && f.data() == stack.data()));
    // `ALLOW` the foods listed, or `BLOCK` them.
    if listed != (settings[0] == 0) {
        return None;
    }
    // `BLOCK_BAD_EFFECTS`.
    if (settings[2] == 0 && HARMFUL.contains(&&*stack.item)) || food_level >= 20 {
        return None;
    }
    let waste = food_level + nutrition - 20;
    match settings[1] {
        // `ALWAYS_EAT`.
        0 => Some((nutrition, saturation)),
        // `FULL_NUTRITION`: none of it goes to waste.
        2 => (waste <= 0).then_some((nutrition, saturation)),
        // `HALF_NUTRITION`: what would go to waste is at most half of it.
        _ => (waste <= nutrition / 2).then_some((nutrition, saturation)),
    }
}

/// `FeedingUpgrade.feedPlayerAndGetHungry`: player `uuid` eats one food from
/// the storage. Whether they still hunger after.
fn feed(game: &mut Game<'_>, uuid: u128, items: &mut [Option<ItemStack>], tier: tb::Tier) -> (bool, bool) {
    let storage = tier.storage;
    let filter = filter_upgrade(items, tier, tb::Tab::Feeding).unwrap_or((Vec::new(), tb::Tab::Feeding.filter_default()));
    let Some(food) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.food) else {
        return (false, false);
    };
    if food >= 20 {
        return (false, false);
    }
    let found = items[..storage]
        .iter()
        .position(|s| s.as_ref().is_some_and(|s| feed_value(game, s, food, &filter).is_some()));
    let Some(slot) = found else {
        return (false, false);
    };
    let Some(stack) = items[slot].as_mut() else {
        return (false, false);
    };
    let Some((nutrition, saturation)) = feed_value(game, stack, food, &filter) else {
        return (false, false);
    };
    // A stew leaves its bowl, a honey bottle its bottle (`finishUsingItem`).
    let remainder = if stack.item.ends_with("_stew") || stack.item.ends_with("_soup") {
        Some("minecraft:bowl")
    } else if &*stack.item == "minecraft:honey_bottle" {
        Some("minecraft:glass_bottle")
    } else {
        None
    };
    stack.count -= 1;
    if stack.count == 0 {
        items[slot] = None;
    }
    if let Some(remainder) = remainder {
        let left = insert_stacked(game, &mut items[..storage], ItemStack::new(remainder, 1));
        if let Some(left) = left {
            let at = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.position);
            if let Some(at) = at {
                game.spawn_item(left, at, [0.0; 3], 40);
            }
        }
    }
    // `FoodData.eat`.
    let mut hungry = false;
    if let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)) {
        g.food = (g.food + nutrition).min(20);
        g.saturation = (g.saturation + saturation).min(g.food as f32);
        hungry = g.food < 20;
    }
    (true, hungry)
}

/// Each server tick, the worn backpacks' upgrades (`BackpackWrapper.tick`),
/// not while their menu is open (it holds their contents then).
fn tick_worn(game: &mut Game<'_>) {
    // The level's time: the server tick is heard once per dimension.
    let tick = u64::try_from(game.world.game_time).unwrap_or(0);
    let dimension = game.dim.id().to_owned();
    let wearers: Vec<(u128, [f64; 3])> = game
        .lan
        .as_ref()
        .map(|h| {
            h.hub.guests.iter().filter(|g| g.placed && g.dimension == dimension).map(|g| (g.uuid, g.position)).collect()
        })
        .unwrap_or_default();
    for (uuid, at) in wearers {
        if worn_menu_open(uuid) {
            continue;
        }
        let Some(mut stack) = worn(game, uuid) else {
            continue;
        };
        let tier = stack_tier(&stack);
        let mut items = contents(&stack, tier.slots());
        let upgrades = upgrades_in(&items, tier);
        if upgrades.is_empty() {
            continue;
        }
        let has = |u: &str| upgrades.iter().any(|x| x == u);
        let mut changed = false;
        if has("travelersbackpack:magnet_upgrade") && tick % cfg().magnet_tick_rate == 0 {
            magnet(game, at, at, Some(uuid), &items, tier);
        }
        // What the wearer touched (`ItemEntityMixin.playerTouch`).
        let touched: Vec<[f64; 3]> = with(|s| {
            let (mine, rest) = std::mem::take(&mut s.touched).into_iter().partition(|t| t.0 == uuid);
            s.touched = rest;
            mine.into_iter().map(|t: (u128, [f64; 3])| t.1).collect()
        });
        if has("travelersbackpack:pickup_upgrade") {
            for at in touched {
                changed |= pickup(game, at.map(|c| c.floor() as i32), &mut items, tier, true);
            }
        }
        if has("travelersbackpack:refill_upgrade") && tick % cfg().refill_tick_rate == 0 {
            changed |= refill(game, uuid, &mut items, tier);
        }
        changed |= smelt_kept(game, &mut items, tier);
        if has("travelersbackpack:feeding_upgrade") {
            let due = with(|s| s.feed_at.get(&uuid).copied().unwrap_or(0)) <= tick;
            if due {
                let (ate, hungry) = feed(game, uuid, &mut items, tier);
                changed |= ate;
                let next = tick + if ate && hungry { FEEDING_HUNGRY_RATE } else { cfg().feeding_tick_rate };
                with(|s| s.feed_at.insert(uuid, next));
            }
        }
        if changed {
            set_contents(&mut stack, items);
            set_worn(game, uuid, Some(&stack));
        }
    }
}

/// A placed backpack's block entity ticks (`BackpackBlockEntity.tick`).
pub(super) fn block_entity_ticks(block: &str, ticks: &mut bool) {
    if tb::is_backpack(block) {
        *ticks = true;
    }
}

/// `BackpackWrapper.tickForBlockEntity`: a placed backpack's upgrades work
/// for it, the magnet pulling items to its front and the feeding feeding
/// the players around; not while its menu is open.
pub(super) fn block_entity_tick(
    game: &mut Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
) {
    if entity.id != tb::BLOCK_ENTITY {
        return;
    }
    sync_fluids(game, pos, entity);
    placed_ability_tick(game, pos, entity);
    if with(|s| s.open.values().any(|o| o.0 == Source::Block(pos))) {
        return;
    }
    let tier = placed_tier(&entity.nbt, &entity.items);
    if entity.items.len() < tier.slots() {
        return;
    }
    let upgrades = upgrades_in(&entity.items, tier);
    if upgrades.is_empty() {
        lantern_light(game, pos, entity, false);
        return;
    }
    let tick = u64::try_from(game.world.game_time).unwrap_or(0);
    let has = |u: &str| upgrades.iter().any(|x| x == u);
    let centre = pos.map(|c| f64::from(c) + 0.5);
    if has("travelersbackpack:magnet_upgrade") && tick % cfg().magnet_tick_rate == 0 {
        // In front of it, a block from its centre on the side it faces.
        let [dx, dz] = match game.native_block_property(pos, "facing").as_deref() {
            Some("south") => [0.0, 1.0],
            Some("east") => [1.0, 0.0],
            Some("west") => [-1.0, 0.0],
            _ => [0.0, -1.0],
        };
        let bottom = [centre[0], f64::from(pos[1]), centre[2]];
        let items = entity.items.clone();
        magnet(game, bottom, [bottom[0] + dx, bottom[1], bottom[2] + dz], None, &items, tier);
    }
    if has("travelersbackpack:pickup_upgrade") && tick % PICKUP_RATE == 0 {
        pickup(game, pos, &mut entity.items, tier, false);
    }
    smelt_kept(game, &mut entity.items, tier);
    lantern_light(game, pos, entity, has("travelersbackpack:lantern_upgrade"));
    if has("travelersbackpack:refill_upgrade") && tick % cfg().refill_tick_rate == 0 {
        // `AABB(pos).inflate(REFILL_RANGE)`: the block and 3 around it.
        let near: Vec<u128> = game
            .lan
            .as_ref()
            .map(|h| {
                h.hub
                    .guests
                    .iter()
                    .filter(|g| {
                        g.placed
                            && !g.dead
                            && g.dimension == game.dim.id()
                            && (0..3).all(|i| (g.position[i] - centre[i]).abs() <= REFILL_RANGE + 0.5)
                    })
                    .map(|g| g.uuid)
                    .collect()
            })
            .unwrap_or_default();
        for uuid in near {
            refill(game, uuid, &mut entity.items, tier);
        }
    }
    if has("travelersbackpack:feeding_upgrade") && tick % FEEDING_HUNGRY_RATE == 0 {
        let near: Vec<u128> = game
            .lan
            .as_ref()
            .map(|h| {
                h.hub
                    .guests
                    .iter()
                    .filter(|g| g.placed && g.dimension == game.dim.id() && (0..3).all(|i| (g.position[i] - centre[i]).abs() <= FEEDING_RANGE + 0.5))
                    .map(|g| g.uuid)
                    .collect()
            })
            .unwrap_or_default();
        for uuid in near {
            let key = u128::from(pos[0] as u32) << 64 | u128::from(pos[2] as u32) << 32 | u128::from(pos[1] as u32);
            let due = with(|s| s.feed_at.get(&(uuid ^ key)).copied().unwrap_or(0)) <= tick;
            if due {
                let (ate, hungry) = feed(game, uuid, &mut entity.items, tier);
                let next = tick + if ate && hungry { FEEDING_HUNGRY_RATE } else { cfg().feeding_tick_rate };
                with(|s| s.feed_at.insert(uuid ^ key, next));
            }
        }
    }
}

// ---- the client ---------------------------------------------------------------------------

/// `AbstractBackpackScreen.TOP_BAR_OFFSET`.
const TOP_BAR: f32 = 17.0;
/// The original's screen textures (`AbstractBackpackScreen`), 256 square.
const BACKGROUND_9: &str = "travelersbackpack:gui/background_9";
const BACKGROUND_11: &str = "travelersbackpack:gui/background_11";
const SLOTS_TEXTURE: &str = "travelersbackpack:gui/slots";
const TANKS_TEXTURE: &str = "travelersbackpack:gui/tanks";
const ICONS: &str = "travelersbackpack:gui/icons";
const TABS: &str = "travelersbackpack:gui/tabs";

/// What the open backpack screen's menu extra data says: its menu type, each
/// upgrade slot's tab (and whether it is open), whether the backpack has a
/// sleeping bag, its tool slots show, it is the settings screen, and the
/// items its kept slots are for.
#[derive(Clone, Default)]
struct ScreenInfo {
    menu: String,
    /// The backpack's storage, upgrade and tool slots.
    sizes: Option<[usize; 3]>,
    tabs: Vec<(tb::Tab, bool)>,
    /// Each upgrade slot's filter in tag mode (the tag selector).
    tags: Vec<bool>,
    bag: bool,
    tools: bool,
    settings: bool,
    memory: Vec<(usize, String)>,
}

thread_local! {
    static SCREEN_INFO: RefCell<ScreenInfo> = RefCell::new(ScreenInfo::default());
    /// Whether shift was held at the last press on a screen.
    static SHIFT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A backpack screen opens: what its menu's extra data says.
pub(super) fn screen_extra(menu: &str, extra: Option<&[u8]>) {
    let extra = extra.unwrap_or_default();
    // The backpack's own sizes first.
    let sizes = match extra {
        [s, u, t, ..] => Some([usize::from(*s), usize::from(*u), usize::from(*t)]),
        _ => None,
    };
    let extra = extra.get(3..).unwrap_or_default();
    let upgrades = match sizes {
        Some([_, u, _]) => u,
        None => tb::menu_kind(menu).map_or(0, |(tier, _)| tier.upgrades),
    };
    let tabs = extra.iter().take(upgrades).map(|&c| (tb::Tab::from_code(c & 0x3F), c & 0x80 != 0)).collect();
    let tags = extra.iter().take(upgrades).map(|&c| c & 0x40 != 0).collect();
    let flag = |k: usize, default: bool| extra.get(upgrades + k).map_or(default, |&b| b != 0);
    let memory = extra
        .get(upgrades + 3..)
        .and_then(|t| std::str::from_utf8(t).ok())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (slot, item) = l.split_once('=')?;
            Some((slot.parse().ok()?, item.to_owned()))
        })
        .collect();
    let info = ScreenInfo { menu: menu.to_owned(), sizes, tabs, tags, bag: flag(0, true), tools: flag(1, false), settings: flag(2, false), memory };
    SCREEN_INFO.with(|t| *t.borrow_mut() = info);
    // A screen opens scrolled to the top.
    SCROLL.with(|s| *s.borrow_mut() = None);
}

/// A press on a screen: whether shift is held (`KeyHelper.isShiftPressed`).
pub(super) fn screen_mouse_pressed(modifiers: u32) {
    SHIFT.with(|s| s.set(modifiers & 1 != 0));
}

/// Where `BackpackScreen` puts things, for a tier and its tabs (the tanks
/// add 22 pixels a side, the wider screen 18).
#[derive(Clone)]
struct Layout {
    tier: tb::Tier,
    /// Each upgrade slot's tab as it shows: a closed one is a plain slot.
    tabs: Vec<tb::Tab>,
    /// Each upgrade slot's upgrade's tab, open or not.
    kinds: Vec<tb::Tab>,
    info: ScreenInfo,
    /// The storage's rows shown (fewer than all on a short window:
    /// `updateDimensions`), and the first of them (`scrollAmount`).
    visible: usize,
    scroll: usize,
}

impl Layout {
    fn of(menu: &str) -> Option<(Self, tb::Opened)> {
        let (tier, opened) = tb::menu_kind(menu)?;
        let info = SCREEN_INFO.with(|t| {
            let t = t.borrow();
            if t.menu == menu { t.clone() } else { ScreenInfo { menu: menu.to_owned(), bag: true, ..Default::default() } }
        });
        let tier = info.sizes.map_or(tier, |[s, u, t]| tier.sized(s, u, t));
        let mut all = info.tabs.clone();
        all.resize(tier.upgrades, (tb::Tab::Plain, false));
        let tabs = all.iter().map(|&(t, open)| if open { t } else { tb::Tab::Plain }).collect();
        let kinds = all.iter().map(|&(t, _)| t).collect();
        let rows = tier.rows();
        // `HEIGHT_WITHOUT_STORAGE` is 114: the rows that fit the window.
        let window = GUI_HEIGHT.with(std::cell::Cell::get);
        let visible = if window > 0.0 && window < 114.0 + 18.0 * rows as f32 {
            (((window - 114.0) / 18.0).floor().max(1.0) as usize).min(rows)
        } else {
            rows
        };
        let scroll = SCROLL.with(|s| s.borrow().as_ref().filter(|(m, _)| m == menu).map_or(0, |(_, k)| *k)).min(rows - visible);
        Some((Self { tier, tabs, kinds, info, visible, scroll }, opened))
    }
    fn tanks(&self) -> bool {
        !self.info.settings && self.kinds.contains(&tb::Tab::Tanks)
    }
    fn tank_offset(&self) -> f32 {
        if self.tanks() { 22.0 } else { 0.0 }
    }
    fn wider(&self) -> f32 {
        if self.tier.extended() { 18.0 } else { 0.0 }
    }
    /// `getWidthAdditions`.
    fn additions(&self) -> f32 {
        self.tank_offset() + self.wider()
    }
    fn per_row(&self) -> usize {
        if self.tier.extended() { 11 } else { 9 }
    }
    fn rows(&self) -> f32 {
        self.visible as f32
    }
    /// `slotsHeight`.
    fn slots_height(&self) -> f32 {
        18.0 * self.rows()
    }
    /// `imageWidth`.
    fn width(&self) -> f32 {
        176.0 + 2.0 * self.wider() + 2.0 * self.tank_offset()
    }
    fn height(&self) -> f32 {
        TOP_BAR + self.slots_height() + 96.0 + 1.0
    }
    /// The upgrade slots' column.
    fn upgrades_x(&self) -> f32 {
        9.0 * 18.0 + 2.0 * self.tank_offset() + 2.0 * self.wider() + 15.0
    }
    /// `getMiddleBar`: under the title bar and the storage's rows.
    fn middle_bar(&self) -> f32 {
        TOP_BAR + self.slots_height() + 1.0
    }
    /// Each upgrade slot's tab, `[x, y, w, h]`, around its slot at its top
    /// left (4 pixels in).
    fn tab_rects(&self) -> Vec<(tb::Tab, [f32; 4])> {
        let x = self.upgrades_x() - 4.0;
        tb::upgrade_slot_ys(&self.tabs)
            .into_iter()
            .zip(&self.tabs)
            .enumerate()
            .map(|(i, (y, &tab))| {
                let [w, h] = tab.size();
                let w = if self.tag(i) { 87.0 } else { w };
                (tab, [x, y - 4.0, w, h])
            })
            .collect()
    }
    /// Upgrade slot `i`'s open tab shows the tag selector.
    fn tag(&self, i: usize) -> bool {
        has_tag_selector(self.tabs.get(i).copied().unwrap_or(tb::Tab::Plain)) && self.info.tags.get(i).copied().unwrap_or(false)
    }
    /// Each tank (`TankWidget`): `[x, y, w, h]`, 18 wide, the storage's height.
    fn tank_columns(&self) -> [[f32; 4]; 2] {
        let h = self.slots_height();
        let right = 195.0 + 2.0 * self.wider();
        [[7.0, TOP_BAR, 18.0, h], [right, TOP_BAR, 18.0, h]]
    }
    /// A storage slot's top left.
    fn storage_slot(&self, i: usize) -> [f32; 2] {
        let [col, row] = [(i % self.per_row()) as f32, (i / self.per_row()) as f32 - self.scroll as f32];
        [8.0 + self.tank_offset() + 18.0 * col, TOP_BAR + 1.0 + 18.0 * row]
    }
    /// Storage slot `i` is in the rows shown.
    fn storage_shown(&self, i: usize) -> bool {
        let row = i / self.per_row();
        row >= self.scroll && row < self.scroll + self.visible
    }
    /// `InventoryScroll`: the bar right of the storage, 4 wide, when it scrolls.
    fn scroll_bar(&self) -> Option<[f32; 4]> {
        (self.visible < self.tier.rows())
            .then(|| [7.0 + self.tank_offset() + 18.0 * self.per_row() as f32, TOP_BAR, 4.0, 18.0 * self.visible as f32])
    }
    /// `SortingButtons`: its box, 50 by 13.
    fn sorting(&self) -> [f32; 2] {
        [2.0 * self.wider() + self.tank_offset() + 120.0, 2.0]
    }
    /// `ToolSlotsWidget`: its button, 10 square.
    fn tools_button(&self) -> [f32; 2] {
        [2.0 * self.wider() + self.tank_offset() + 110.0, 5.0]
    }
    /// `SettingsWidget`, right of the panel.
    fn settings_tab(&self) -> [f32; 2] {
        [self.width() - 3.0, 4.0]
    }
}

/// What the menu's data slots say past the tabs' (`settings_data`).
#[derive(Clone, Default)]
struct Live {
    sort: i32,
    tools: bool,
    more: bool,
    tab: u8,
    matching: bool,
    visible: bool,
    unsortable: Vec<bool>,
    memory: Vec<bool>,
    matched: Vec<bool>,
    /// Which upgrade slots hold an upgrade, which are on, which can be off.
    upgrades: Vec<bool>,
    enabled: Vec<bool>,
    switchable: Vec<bool>,
    /// The ability slider shown, its ability on, its cooldown in seconds.
    slider: bool,
    ability_on: bool,
    cooldown: i32,
    /// The trash bin on, its tanks and slots chosen (storage, then inventory).
    trash: bool,
    trash_tanks: [bool; 2],
    trash_storage: Vec<bool>,
    trash_inventory: Vec<bool>,
}

impl Live {
    fn of(layout: &Layout, data: &[i32]) -> Self {
        let base = 5 * layout.tier.upgrades;
        let get = |k: usize| data.get(base + k).copied().unwrap_or(0);
        let storage = layout.tier.storage;
        let words = storage.div_ceil(16);
        let bits = |from: usize| (0..storage).map(|s| get(from + s / 16) & 0xFFFF & (1 << (s % 16)) != 0).collect();
        let flags = get(1);
        let up = get(2 + 3 * words);
        let on = get(3 + 3 * words);
        let switch = get(4 + 3 * words);
        let ability = get(5 + 3 * words);
        Self {
            sort: get(0),
            tools: flags & 1 != 0,
            more: flags & 2 != 0,
            tab: ((flags >> 3) & 3) as u8,
            matching: flags & 32 != 0,
            visible: flags & 64 != 0 || data.len() <= base,
            unsortable: bits(2),
            memory: bits(2 + words),
            matched: bits(2 + 2 * words),
            upgrades: (0..layout.tier.upgrades).map(|i| up & (1 << i) != 0).collect(),
            enabled: (0..layout.tier.upgrades).map(|i| on & (1 << i) != 0).collect(),
            switchable: (0..layout.tier.upgrades).map(|i| switch & (1 << i) != 0).collect(),
            slider: ability & 1 != 0,
            ability_on: ability & 2 != 0,
            cooldown: get(6 + 3 * words),
            trash: get(7 + 3 * words) & 1 != 0,
            trash_tanks: [get(7 + 3 * words) & 2 != 0, get(7 + 3 * words) & 4 != 0],
            trash_storage: (0..storage).map(|s| get(8 + 3 * words + s / 16) & 0xFFFF & (1 << (s % 16)) != 0).collect(),
            trash_inventory: (0..36).map(|k| get(8 + 4 * words + k / 16) & 0xFFFF & (1 << (k % 16)) != 0).collect(),
        }
    }
}

/// The backpack screen of menu type `menu`, laid out as `BackpackBaseMenu`
/// places its slots: the storage in rows of nine below the title bar (eleven
/// past 81 slots, the wider screen), the upgrade slots in a column right of
/// the panel, each with its tab open below it (a tab's slots inside it), the
/// tool slots in a column left of it when shown, then the player's
/// inventory. With tanks, the storage moves right of the left tank. The
/// settings screen (`BackpackSettingsScreen`) has the storage and the
/// inventory only.
pub(super) fn screen(menu: &str, screen: &mut Option<NativeScreen>) {
    let Some((layout, _)) = Layout::of(menu) else {
        return;
    };
    let tier = layout.tier;
    let settings = layout.info.settings;
    let mut slots = Vec::with_capacity(tier.menu_slots());
    for i in 0..tier.storage {
        slots.push(layout.storage_shown(i).then(|| layout.storage_slot(i)));
    }
    let rects = layout.tab_rects();
    // `addBackpackUpgradeSlots`.
    for (_, [x, y, _, _]) in &rects {
        slots.push((!settings).then_some([x + 4.0, y + 4.0]));
    }
    // `addBackpackToolSlots`, shown or not (`ToolSlotItemHandler.isActive`).
    for i in 0..tier.tools {
        slots.push((!settings && layout.info.tools).then_some([-14.0, 18.0 + 18.0 * i as f32]));
    }
    // Each tab's own slots, the rest of its block hidden.
    for (i, (tab, [x, y, _, _])) in rects.iter().enumerate() {
        // The tag selector's one slot, for the item whose tags are added.
        let own = if layout.tag(i) { vec![[64.0, 23.0]] } else { tab.slots() };
        for k in 0..tb::TAB_SLOTS {
            slots.push(own.get(k).filter(|_| !settings).map(|[sx, sy]| [x + sx, y + sy]));
        }
    }
    // The slots past this backpack's, hidden.
    slots.resize(tb::MENU_SLOTS, None);
    *screen = Some(NativeScreen {
        width: layout.width(),
        height: layout.height(),
        slots,
        inventory_y: layout.slots_height() + 7.0 + 25.0,
        inventory_x: 8.0 + layout.additions(),
        own_labels: true,
        own_background: true,
    });
}

// ---- the open backpack's slot ------------------------------------------------------------

/// `BackpackBaseMenu`'s locked slot: while a backpack's menu is open from the
/// hand, a click on its slot, or a number key swapping it, does nothing
/// (it would take the backpack away from what the menu writes back to).
pub(super) fn lock_slot(
    host: &super::lan::LanHost,
    conn: u32,
    p: &mut horizon_protocol::packets::play::ContainerClick,
) {
    let Some(g) = host.hub.guest(conn) else {
        return;
    };
    let Some((Source::Hand(slot), _, window)) = with(|s| s.open.get(&g.uuid).cloned()) else {
        return;
    };
    let Some(menu) = g.menu.as_ref().filter(|m| m.window == window && p.container == window) else {
        return;
    };
    // The menu's own slots, then the inventory's 27, then the hotbar.
    let locked = if slot < 9 { menu.size + 27 + slot } else { menu.size + slot - 9 };
    let clicked = usize::try_from(p.slot).is_ok_and(|s| s == locked);
    let swapped = p.input == 2 && usize::try_from(p.button).is_ok_and(|b| b == slot);
    if clicked || swapped {
        (p.slot, p.button, p.input) = (-999, 3 << 2, 5);
    }
}

// ---- the worn backpack, on the client -----------------------------------------------------

thread_local! {
    /// Who wears which backpack, as the server told (`<name>` to its id).
    static WEARING: RefCell<HashMap<String, Worn>> = RefCell::new(HashMap::new());
    /// Who wears a backpack with its lantern on, as the server told (`<name>`).
    static LANTERNS: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
    /// Better Dynamic Lights is in this build (asked once).
    static BETTER_LIGHTS: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    /// Its settings for the lanterns as last read, and when.
    static LIGHT_RULES: std::cell::Cell<Option<(std::time::Instant, Option<LightRules>)>> = const { std::cell::Cell::new(None) };
}

// ---- the worn lantern's light (`BackpackEntityLuminance`) ----

/// Better Dynamic Lights (DeportedToHell's): while it is on, it gathers the
/// frame's dynamic lights in Horizon's place, by its own settings.
const BETTER_LIGHTS_ID: &str = "dynamiclights";
/// `LanternUpgrade.writeToRenderData`: a lantern's light.
const LANTERN_LEVEL: u8 = 15;

/// How the lanterns light this frame: for this player, for the others,
/// how brightly, how far from the camera and how many lights at most.
#[derive(Clone, Copy)]
struct LightRules {
    own: bool,
    others: bool,
    level: f32,
    range: f64,
    most: usize,
}

/// The dynamic lights' rules now: Better Dynamic Lights' when it is in the
/// build and on (its main switch, "Your hands", "Players", brightness, range
/// and most sources), else Horizon's own extra's. None: no dynamic lights.
fn light_rules(game: &Game<'_>) -> Option<LightRules> {
    let better = BETTER_LIGHTS.with(std::cell::Cell::get).unwrap_or(false);
    if !(better && game.native_on(BETTER_LIGHTS_ID)) {
        return game.settings.extras.dynamic_lights.then_some(LightRules {
            own: true,
            others: true,
            level: f32::from(LANTERN_LEVEL),
            range: 96.0,
            most: horizon_client::dynamic_light::MAX,
        });
    }
    // Its settings are files: read again at most once a second.
    let now = std::time::Instant::now();
    if let Some((at, rules)) = LIGHT_RULES.with(std::cell::Cell::get)
        && now.duration_since(at).as_secs_f32() < 1.0
    {
        return rules;
    }
    let get = |key: &str| game.native_setting(BETTER_LIGHTS_ID, key);
    let flag = |key: &str| get(key).as_deref() != Some("false");
    let number = |key: &str, default: u8| get(key).and_then(|v| v.parse::<u8>().ok()).unwrap_or(default);
    let rules = flag("enabled").then(|| LightRules {
        own: flag("own_hands"),
        others: flag("players"),
        level: f32::from(LANTERN_LEVEL) * f32::from(number("brightness", 100)) / 100.0,
        range: f64::from(number("range", 64)),
        most: usize::from(number("max_sources", 16)),
    });
    LIGHT_RULES.with(|r| r.set(Some((now, rules))));
    rules
}

/// Right after the frame's dynamic lights are gathered: each player wearing
/// a backpack with its lantern on gives a lantern's light where a held one
/// would (the hands' height), among the nearest the frame keeps.
pub(super) fn lantern_lights(game: &mut Game<'_>, camera: glam::DVec3, alpha: f64) {
    if BETTER_LIGHTS.with(std::cell::Cell::get).is_none() {
        let mut mods = Vec::new();
        game.native_mods(&mut mods);
        let better = mods.iter().any(|m| m.id == BETTER_LIGHTS_ID);
        BETTER_LIGHTS.with(|b| b.set(Some(better)));
    }
    if LANTERNS.with(|l| l.borrow().is_empty()) {
        return;
    }
    let Some(rules) = light_rules(game) else {
        return;
    };
    let lit = |name: &str| LANTERNS.with(|l| l.borrow().contains(name));
    let mut sources = Vec::new();
    if rules.own
        && lit(&game.own_profile_name())
        && let Some(p) = game.player.as_ref().filter(|p| !p.spectator)
    {
        let feet = game.previous_position.lerp(p.position, game.clock.camera_fraction());
        sources.push(glam::DVec3::new(feet.x, feet.y + 1.2, feet.z));
    }
    if rules.others {
        for m in game.mobs.iter().filter(|m| !m.removed && !m.extra.spectator && m.kind.id == "minecraft:player") {
            if game.remote_player_name(m.id).is_some_and(lit) {
                let feet = m.previous.lerp(m.position, alpha);
                sources.push(glam::DVec3::new(feet.x, feet.y + f64::from(m.height()) * 0.6, feet.z));
            }
        }
    }
    let reach = rules.range * rules.range;
    let mut lights = std::mem::take(&mut game.dynamic_lights);
    for at in sources.into_iter().filter(|at| at.distance_squared(camera) <= reach) {
        let p = (at - camera).as_vec3();
        // A light there already (a torch in hand): the brighter of the two.
        let same = lights.iter_mut().find(|l| (l[0] - p.x).powi(2) + (l[1] - p.y).powi(2) + (l[2] - p.z).powi(2) < 0.01);
        match same {
            Some(l) => l[3] = l[3].max(rules.level),
            None => lights.push([p.x, p.y, p.z, rules.level]),
        }
    }
    // The nearest first (those gathered first on a tie), as many as kept.
    let distance = |l: &[f32; 4]| l[0] * l[0] + l[1] * l[1] + l[2] * l[2];
    lights.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
    lights.truncate(rules.most);
    game.dynamic_lights = lights;
}

/// A player's worn backpack as the client draws it: its block, its tanks
/// shown, their fluids.
#[derive(Clone)]
struct Worn {
    backpack: String,
    tanks: bool,
    fluids: Option<[(i32, f32); 2]>,
    bag: String,
    dye: Option<i32>,
    /// The tools strapped to it (`StackModelPart`): the first, then the last.
    tools: Vec<ItemStack>,
}

/// `KeybindHandler.OPEN_BACKPACK` (B by default): while playing, the server
/// opens the worn backpack, else the first in the inventory.
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if key == "key.keyboard.left.shift" || key == "key.keyboard.right.shift" {
        SHIFT_HELD.with(|s| s.set(pressed));
    }
    if !pressed {
        if key == game.native_mod_key(MOD_ID, "cycle_tool", "key.keyboard.z") {
            radial_released(game);
        }
        return;
    }
    // `SORT_BACKPACK` (unbound at first): sorts the open backpack, shift its way.
    if game.native_screen_shown() && key == game.native_mod_key(MOD_ID, "sort", "key.keyboard.unknown") {
        let shift = SHIFT_HELD.with(std::cell::Cell::get);
        let _ = game.native_net_send_to_server(if shift { SORT_SHIFT_CHANNEL } else { SORT_CHANNEL }, &[]);
        return;
    }
    if !game.native_playing() {
        // `allowOpeningFromSlot`: on an inventory screen, the key over a backpack opens it.
        if cfg().allow_opening_from_slot && key == game.native_mod_key(MOD_ID, "inventory", "key.keyboard.b") {
            let hovered = HOVERED_SLOT.with(std::cell::Cell::get).filter(|(_, at)| at.elapsed().as_millis() < 200).map(|(i, _)| i);
            if let Some(slot) = hovered.filter(|&i| i < 36)
                && game.inventory.get(slot).cloned().flatten().is_some_and(|s| tb::is_backpack(&s.item))
            {
                let _ = game.native_net_send_to_server(OPEN_SLOT_CHANNEL, &[slot as u8]);
            }
        }
        return;
    }
    // `ABILITY` (unbound at first): the worn backpack's ability on or off.
    if key == game.native_mod_key(MOD_ID, "ability", "key.keyboard.unknown") {
        let me = game.own_profile_name();
        let design = WEARING.with(|w| w.borrow().get(&me).map(|w| design_of(&w.backpack).to_owned()));
        if design.is_some_and(|d| ability(&d).is_some()) {
            // The server answers with the state it has now, in the action bar.
            let _ = game.native_net_send_to_server(ABILITY_TOGGLE_CHANNEL, &[]);
        }
    }
    // `TOGGLE_UPGRADE_0`..`3` (unbound at first): an upgrade slot of the worn backpack on or off.
    for (i, channel) in TOGGLE_UPGRADE_CHANNELS.iter().enumerate() {
        if key == game.native_mod_key(MOD_ID, &format!("toggle_upgrade_{i}"), "key.keyboard.unknown") {
            let _ = game.native_net_send_to_server(channel, &[]);
        }
    }
    if key == game.native_mod_key(MOD_ID, "inventory", "key.keyboard.b") {
        let _ = game.native_net_send_to_server(OPEN_CHANNEL, &[]);
    }
    if key == game.native_mod_key(MOD_ID, "cycle_tool", "key.keyboard.z") {
        open_radial(game);
    }
}

thread_local! {
    /// Whether shift is held while playing (`KeyHelper.isShiftPressed`).
    static SHIFT_HELD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The server told who wears what (`ClientboundSyncAttachmentPacket`).
pub(super) fn net_from_server(channel: &str, data: &[u8]) {
    if channel == JUKEBOX_CHANNEL {
        if let Some((entity, sound)) = std::str::from_utf8(data).ok().and_then(|t| t.split_once('\t'))
            && let Ok(entity) = entity.parse::<i32>()
        {
            SONGS.with(|s| s.borrow_mut().push((entity, sound.to_owned())));
        }
        return;
    }
    if channel == FLUID_WARNING_CHANNEL {
        let lines = std::str::from_utf8(data).unwrap_or_default().lines().map(str::to_owned).collect();
        FLUID_WARNING.with(|w| *w.borrow_mut() = Some((std::time::Instant::now(), lines)));
        return;
    }
    if channel == TAGS_CHANNEL {
        tag_lists_received(data);
        return;
    }
    if channel == TOOLS_CHANNEL {
        tools_received(data);
        return;
    }
    if channel == MESSAGE_CHANNEL {
        if let Ok(text) = std::str::from_utf8(data) {
            PENDING_MESSAGES.with(|m| m.borrow_mut().push(text.to_owned()));
        }
        return;
    }
    if channel == ABILITY_CHANNEL {
        // `spiderAbility`: this player climbs walls.
        horizon_client::motion::travelersbackpack_spider::set(data == b"spider");
        return;
    }
    if channel != WORN_CHANNEL {
        return;
    }
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Some((name, rest)) = text.split_once('\n') else {
        return;
    };
    let mut lines = rest.splitn(3, '\n');
    let worn = lines.next().unwrap_or_default();
    let lantern = lines.next().unwrap_or("0");
    let tools: Vec<ItemStack> = lines
        .next()
        .unwrap_or_default()
        .split('\t')
        .filter(|t| !t.is_empty())
        .filter_map(|t| horizon_world::nbt::parse_snbt(t).ok().and_then(|(tag, _)| horizon_items::item_nbt::item_from_nbt(&tag)))
        .collect();
    LANTERNS.with(|l| {
        let mut l = l.borrow_mut();
        if lantern == "1" {
            l.insert(name.to_owned());
        } else {
            l.remove(name);
        }
    });
    // `<id>\t<tanks>\t<left,fill,right,fill>\t<bag>\t<dye>`.
    let mut parts = worn.split('\t');
    let backpack = parts.next().unwrap_or_default();
    let tanks = parts.next() == Some("true");
    let numbers: Vec<i32> = parts.next().unwrap_or_default().split(',').filter_map(|n| n.parse().ok()).collect();
    let fluids = (numbers.len() == 4).then(|| [(numbers[0], numbers[1] as f32 / 1000.0), (numbers[2], numbers[3] as f32 / 1000.0)]);
    let bag = parts.next().unwrap_or("red").to_owned();
    let dye = parts.next().and_then(|d| d.parse().ok());
    WEARING.with(|w| {
        let mut w = w.borrow_mut();
        if tb::is_backpack(backpack) {
            w.insert(name.to_owned(), Worn { backpack: backpack.to_owned(), tanks, fluids, bag, dye, tools });
        } else {
            w.remove(name);
        }
    });
}

/// The game poses players for `mob_layer`.
pub(super) fn render_listened(listened: &mut bool) {
    *listened = true;
}

/// `BackpackLayer`: the worn backpack on a player's back. The player's frame
/// starts at the feet, +z the way the body faces; the backpack's model (the
/// block's, facing south: its straps toward +z) stands with its straps on
/// the back of the body, two pixels behind its middle.
pub(super) fn mob_layer(
    game: &Game<'_>,
    mob: &super::native_mob_draw::NativeMobRender<'_>,
    who: Option<&super::native_mob_draw::NativePlayerRender<'_>>,
    draw: &mut super::native_block_entity_draw::BlockEntityDraw,
) {
    let Some(who) = who else {
        mob_backpack_layer(game, mob, draw);
        return;
    };
    let Some(Worn { backpack, tanks, fluids, bag, dye, tools }) = WEARING.with(|w| w.borrow().get(who.name).cloned()) else {
        return;
    };
    draw.push();
    draw.translate([-0.5, 0.82, -0.125 - 11.8 / 16.0]);
    let tanks = if tanks { "true" } else { "false" };
    if backpack == STANDARD {
        let dyed = if dye.is_some() { "true" } else { "false" };
        game.native_draw_block(draw, &backpack, &[("bag", &bag), ("dyed", dyed), ("facing", "south"), ("tanks", tanks)]);
    } else {
        game.native_draw_block(draw, &backpack, &[("bag", &bag), ("facing", "south"), ("tanks", tanks)]);
    }
    if let Some(colour) = dye.filter(|_| backpack == STANDARD) {
        draw_dyed_body(game, draw, colour, model_turn("south"));
    }
    if let Some(levels) = fluids.filter(|_| tanks == "true") {
        draw_tank_fluids(game, draw, levels, model_turn("south"));
    }
    // `StackModelPart` (`renderTools`): the first tool across the top of the
    // backpack's outside, the last one standing at its side, half size.
    if cfg().render_tools {
        use horizon_assets::item_model::DisplayContext;
        let context = |t: &ItemStack| {
            if matches!(&*t.item, "minecraft:trident" | "minecraft:spyglass") { DisplayContext::Gui } else { DisplayContext::None }
        };
        if let Some(upper) = tools.first() {
            draw.push();
            draw.translate([0.5, 0.42, 4.6 / 16.0]);
            draw.rotate_about([0.0, 0.0, 1.0], 45.0, [0.0, 0.0, 0.0]);
            draw.scale([0.5, 0.5, 0.5]);
            game.native_draw_item(draw, upper, context(upper), None, 0);
            draw.pop();
        }
        if let Some(lower) = tools.get(1) {
            draw.push();
            draw.translate([12.6 / 16.0, 0.25, 7.0 / 16.0]);
            draw.rotate_about([0.0, 1.0, 0.0], 90.0, [0.0, 0.0, 0.0]);
            draw.rotate_about([0.0, 0.0, 1.0], 45.0, [0.0, 0.0, 0.0]);
            draw.scale([0.5, 0.5, 0.5]);
            game.native_draw_item(draw, lower, context(lower), None, 0);
            draw.pop();
        }
    }
    draw.pop();
}

/// A clickable thing on the backpack screen: its menu button id (the server's
/// `menu_button`), its box from the panel's top left, and what it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Widget {
    Equip,
    Unequip,
    SleepingBag,
    More,
    Sort(u8),
    Tools,
    Settings,
    Remove(usize),
    Jukebox(usize, bool),
    Filter(usize, usize),
    Enable(usize),
    Unsortables,
    SelectAll,
    RemoveAll,
    Memory,
    Match,
    Visibility,
    Ability,
    /// The void upgrade's trash bin, in its open tab.
    Trash,
    /// A filter's tag list (tag mode).
    Tags(usize),
}

/// The settings screen's tabs (`SettingsWidgetBase`), one under the other
/// right of the panel, each 24 square, an open one bigger: the unsortables
/// (48 by 46), the memory (30 by 46), the visibility (worn only).
fn settings_tabs(layout: &Layout, opened: tb::Opened, live: &Live) -> Vec<(Widget, [f32; 4])> {
    let [x, mut y] = layout.settings_tab();
    y += 25.0;
    let mut tabs = Vec::new();
    for (w, open, size) in [(Widget::Unsortables, live.tab == 1, [48.0, 46.0]), (Widget::Memory, live.tab == 2, [30.0, 46.0])] {
        let [tw, th] = if open { size } else { [24.0, 24.0] };
        tabs.push((w, [x, y, tw, th]));
        y += th + 1.0;
    }
    if opened == tb::Opened::Worn {
        tabs.push((Widget::Visibility, [x, y, 24.0, 24.0]));
    }
    tabs
}

/// Everything clickable on menu `menu`'s screen, `[x, y, w, h]` from the
/// panel's top left.
fn widgets(menu: &str, data: &[i32]) -> Vec<(Widget, [f32; 4])> {
    let Some((layout, opened)) = Layout::of(menu) else {
        return Vec::new();
    };
    let live = Live::of(&layout, data);
    let [sx, sy] = layout.settings_tab();
    let mut out = vec![(Widget::Settings, [sx + 3.0, sy + 3.0, 18.0, 18.0])];
    if layout.info.settings {
        for (w, [x, y, tw, _]) in settings_tabs(&layout, opened, &live) {
            out.push((w, [x + 3.0, y + 3.0, 18.0, 18.0]));
            match w {
                Widget::Unsortables if tw > 24.0 => {
                    out.push((Widget::SelectAll, [x + 6.0, y + 22.0, 18.0, 18.0]));
                    out.push((Widget::RemoveAll, [x + 24.0, y + 22.0, 18.0, 18.0]));
                }
                Widget::Memory if tw > 24.0 => out.push((Widget::Match, [x + 6.0, y + 22.0, 18.0, 18.0])),
                _ => {}
            }
        }
        return out;
    }
    // `SortingButtons`: sort, quick stack, to the backpack, to the player.
    let [bx, by] = layout.sorting();
    for i in 0..4u8 {
        out.push((Widget::Sort(i), [bx + 2.0 + 12.0 * f32::from(i), by + 3.0, 10.0, 10.0]));
    }
    let [tx, ty] = layout.tools_button();
    out.push((Widget::Tools, [tx, ty, 10.0, 10.0]));
    // The buttons on the bar under the storage (`initButtons`).
    let (x, y) = (layout.additions(), layout.middle_bar());
    let at = |w: Widget, dx: f32| (w, [x + dx, y, 12.0, 12.0]);
    match opened {
        tb::Opened::Held => out.push(at(Widget::Equip, 157.0)),
        tb::Opened::Worn => {
            out.push(at(Widget::More, 157.0));
            if live.more {
                out.push(at(Widget::Unequip, 145.0));
                if layout.info.bag {
                    out.push(at(Widget::SleepingBag, 133.0));
                }
                // `AbilitySliderButton`: after the others.
                if live.slider {
                    out.push(at(Widget::Ability, if layout.info.bag { 121.0 } else { 133.0 }));
                }
            }
        }
        // The more button alone shows nothing: none without a bag or a slider.
        tb::Opened::Placed if layout.info.bag || live.slider => {
            out.push(at(Widget::More, 157.0));
            if live.more {
                if layout.info.bag {
                    out.push(at(Widget::SleepingBag, 145.0));
                }
                if live.slider {
                    out.push(at(Widget::Ability, if layout.info.bag { 133.0 } else { 145.0 }));
                }
            }
        }
        tb::Opened::Placed => {}
    }
    // An open tab's remove button; the jukebox's play and stop.
    for (i, (tab, [tx, ty, tw, _])) in layout.tab_rects().into_iter().enumerate() {
        if tab == tb::Tab::Plain {
            // `renderEnableButton`: the switch at a closed upgrade's right.
            if live.switchable.get(i).copied().unwrap_or(false) {
                out.push((Widget::Enable(i), [tx + tw, ty + 6.0, 4.0, 13.0]));
            }
            continue;
        }
        out.push((Widget::Remove(i), [tx + tw - 21.0, ty + 3.0, 18.0, 18.0]));
        if tab == tb::Tab::Void {
            out.push((Widget::Trash, [tx + 25.0, ty + 4.0, 16.0, 16.0]));
        }
        if tab.filter_states().is_some() {
            // In tag mode the third (components) is hidden.
            for k in 0..if layout.tag(i) { 2 } else { 3 } {
                out.push((Widget::Filter(i, k), [tx + 6.0 + 18.0 * k as f32, ty + 22.0, 18.0, 18.0]));
            }
            if layout.tag(i) {
                out.push((Widget::Tags(i), [tx + 9.0, ty + 44.0, 73.0, 52.0]));
            }
        }
        if tab == tb::Tab::Jukebox {
            out.push((Widget::Jukebox(i, true), [tx + 24.0, ty + 22.0, 18.0, 18.0]));
            out.push((Widget::Jukebox(i, false), [tx + 42.0, ty + 22.0, 18.0, 18.0]));
        }
    }
    out
}

fn inside([x, y, w, h]: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= x && at[0] < x + w && at[1] >= y && at[1] < y + h
}

/// A press on a widget sends its id to the server (`menu_button` there):
/// shift held, the sort buttons' and the sleeping bag's other id.
pub(super) fn screen_click(menu: &str, data: &[i32], at: [f32; 2], right: bool, pressed: &mut Option<i32>) {
    if right || pressed.is_some() {
        return;
    }
    if let Some((layout, _)) = Layout::of(menu)
        && !layout.info.settings
        && layout.tanks()
        && let Some(side) = layout.tank_columns().iter().position(|r| inside(*r, at))
    {
        *pressed = Some(96 + side as i32);
        return;
    }
    let shift = SHIFT.with(std::cell::Cell::get);
    let Some(&(w, _)) = widgets(menu, data).iter().find(|(_, r)| inside(*r, at)) else {
        return;
    };
    let Some((layout, _)) = Layout::of(menu) else {
        return;
    };
    *pressed = Some(match w {
        Widget::Equip => 0,
        Widget::Unequip => 1,
        Widget::SleepingBag => {
            if shift {
                3
            } else {
                2
            }
        }
        Widget::More => 41,
        Widget::Sort(i) => 30 + i32::from(i) + if shift { 4 } else { 0 },
        Widget::Tools => 40,
        Widget::Settings => {
            if layout.info.settings {
                43
            } else {
                42
            }
        }
        Widget::Remove(i) => 50 + i as i32,
        Widget::Jukebox(i, _) => 20 + i as i32,
        Widget::Filter(i, k) => 60 + 3 * i as i32 + k as i32,
        Widget::Enable(i) => 80 + i as i32,
        Widget::Unsortables => 44,
        Widget::Memory => 45,
        Widget::SelectAll => 46,
        Widget::RemoveAll => 47,
        Widget::Match => 48,
        Widget::Visibility => 49,
        Widget::Trash => 95,
        Widget::Tags(i) => 200 + 50 * i as i32 + selected_tag(i) as i32,
        Widget::Ability => 90,
    });
}

fn tr(game: &Game<'_>, key: &str) -> String {
    game.translate(key, key)
}

/// The tooltip of what the cursor is over (`renderTooltip`s).
pub(super) fn screen_tooltip(game: &Game<'_>, menu: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    let Some((layout, _)) = Layout::of(menu) else {
        return;
    };
    let live = Live::of(&layout, data);
    let split = |key: &str| tr(game, key).split('\n').map(str::to_owned).collect::<Vec<_>>();
    if let Some(&(w, _)) = widgets(menu, data).iter().find(|(_, r)| inside(*r, at)) {
        let key = |k: &str| format!("screen.travelersbackpack.{k}");
        let found: Vec<String> = match w {
            Widget::Equip => vec![tr(game, &key("equip"))],
            Widget::Unequip => vec![tr(game, &key("unequip"))],
            Widget::SleepingBag => vec![tr(game, &key("use_sleeping_bag")), tr(game, &key("detach_sleeping_bag"))],
            Widget::Ability => {
                let state = if live.ability_on { "ability_enabled" } else { "ability_disabled" };
                let second = if live.cooldown > 0 {
                    let time = format!("{}:{:02}", live.cooldown / 60, live.cooldown % 60);
                    tr(game, &key("ability_cooldown")).replacen("%s", &time, 1)
                } else {
                    tr(game, &key("ability_ready"))
                };
                vec![tr(game, &key(state)), second]
            }
            Widget::More => vec![tr(game, &key("show_more_buttons"))],
            Widget::Sort(0) => {
                let way = ["category", "name", "count"][live.sort.clamp(0, 2) as usize];
                vec![tr(game, &key("sort")), tr(game, &key("sort_shift")), tr(game, &key(&format!("sort_{way}")))]
            }
            Widget::Sort(1) => vec![tr(game, &key("quick_stack")), tr(game, &key("quick_stack_shift"))],
            Widget::Sort(2) => vec![tr(game, &key("transfer_to_backpack")), tr(game, &key("transfer_to_backpack_shift"))],
            Widget::Sort(_) => vec![tr(game, &key("transfer_to_player"))],
            Widget::Tools => vec![tr(game, &key(if live.tools { "hide_tool_slots" } else { "show_tool_slots" }))],
            Widget::Settings => vec![tr(game, &key(if layout.info.settings { "settings_back" } else { "settings" }))],
            Widget::Remove(_) | Widget::Jukebox(..) => Vec::new(),
            Widget::Filter(i, k) => {
                let state = data.get(5 * i + k).copied().unwrap_or(0).max(0) as usize;
                filter_tooltips(layout.tabs[i], k).get(state).map(|t| vec![tr(game, &key(t))]).unwrap_or_default()
            }
            Widget::Enable(i) => {
                vec![if live.enabled.get(i).copied().unwrap_or(true) { "Disable Upgrade" } else { "Enable Upgrade" }.to_owned()]
            }
            Widget::Unsortables => split(&key("unsortable")),
            Widget::SelectAll => {
                let mut l = split(&key("unsortable"));
                l.push(tr(game, &key("select_all")));
                l
            }
            Widget::RemoveAll => {
                let mut l = split(&key("unsortable"));
                l.push(tr(game, &key("remove_all")));
                l
            }
            Widget::Memory => split(&key("memory")),
            Widget::Match => {
                vec![tr(game, &key(if live.matching { "filter_match_components" } else { "filter_ignore_components" }))]
            }
            Widget::Visibility => vec![tr(game, &key("toggle_visibility"))],
            Widget::Tags(i) => tag_tooltip(game, i),
            Widget::Trash => {
                let chosen = live.trash_storage.iter().chain(&live.trash_inventory).any(|&b| b) || live.trash_tanks.iter().any(|&b| b);
                if live.trash && chosen {
                    vec![format!("§c{}", tr(game, &key("void_upgrade_trash_bin_drop")))]
                } else {
                    let mut lines = split(&key("void_upgrade_trash_bin"));
                    if !layout.tanks() && lines.len() > 2 {
                        lines.remove(2);
                    }
                    lines
                }
            }
        };
        lines.extend(found);
        return;
    }
    // A tank: its fluid and how much (`TankWidget.getTankTooltip`).
    if let Some(i) = layout.kinds.iter().position(|&t| t == tb::Tab::Tanks).filter(|_| layout.tanks()) {
        for (side, rect) in layout.tank_columns().into_iter().enumerate() {
            if !inside(rect, at) {
                continue;
            }
            let get = |k: usize| data.get(5 * i + k).copied().unwrap_or(0);
            let (fluid, amount, capacity) = (get(2 * side), get(2 * side + 1) & 0xFFFF, get(4) & 0xFFFF);
            match fluid {
                1 => lines.push(tr(game, "block.minecraft.water")),
                2 => lines.push(tr(game, "block.minecraft.lava")),
                3 => lines.push(tr(game, "fluid_type.travelersbackpack.milk")),
                4 => lines.push(tr(game, "fluid_type.travelersbackpack.potion")),
                _ => {
                    lines.push(tr(game, "screen.travelersbackpack.empty"));
                    continue;
                }
            }
            lines.push(format!("§7{}/{} mB", mb(amount), mb(capacity)));
        }
    }
}

/// The screen (`BackpackScreen.renderScreen`, `BackpackSettingsScreen`), drawn
/// from the original's textures under the items: the tabs and upgrade
/// slots, the panel, the tanks, the storage's slots, the sort and tool
/// buttons, the settings tab, the bar's buttons; the unsortable and kept
/// slots marked; the title and the inventory's label.
pub(super) fn screen_draw(
    game: &Game<'_>,
    menu: &str,
    data: &[i32],
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
) {
    let Some((layout, opened)) = Layout::of(menu) else {
        return;
    };
    let live = Live::of(&layout, data);
    let [ox, oy] = origin;
    let cursor = game.native_cursor();
    let mouse = [cursor[0] - ox, cursor[1] - oy];
    TRASH_SHOWN.with(|t| t.set(live.trash && !layout.info.settings));
    LAST_SCREEN.with(|l| *l.borrow_mut() = Some((menu.to_owned(), data.to_vec())));
    // `blit`: part `[u, v, w, h]` of a texture at `(x, y)` from the panel's top left.
    let blit = |batch: &mut horizon_client::hud::HudBatch, texture: &str, x: f32, y: f32, [u, v, w, h]: [f32; 4]| {
        if w > 0.0 && h > 0.0 {
            game.native_sprite(batch, texture, [u, v, w, h], [ox + x, oy + y, w, h]);
        }
    };
    let settings = layout.info.settings;
    let get = |k: usize| data.get(k).copied().unwrap_or(0);

    // The upgrade slots, and the open tabs (`UpgradeSlot`, `UpgradeWidgetBase.renderBg`).
    if !settings {
        for (i, (tab, [x, y, w, h])) in layout.tab_rects().into_iter().enumerate() {
            match tab {
                tb::Tab::Plain => {
                    blit(batch, ICONS, x, y, [0.0, 0.0, 24.0, 24.0]);
                    if !live.upgrades.get(i).copied().unwrap_or(false) {
                        blit(batch, ICONS, x + 3.0, y + 3.0, [24.0, 0.0, 18.0, 18.0]);
                    }
                    if live.switchable.get(i).copied().unwrap_or(false) {
                        let on = live.enabled.get(i).copied().unwrap_or(true);
                        blit(batch, ICONS, x + w, y + 6.0, [18.0, if on { 24.0 } else { 37.0 }, 4.0, 13.0]);
                        if inside([x + w, y + 6.0, 4.0, 13.0], mouse) {
                            let hy = if on { 7.0 } else { 1.0 };
                            game.native_fill(batch, [ox + x + w, oy + y + 6.0 + hy, 3.0, 5.0], 0x80FF_FFFF);
                        }
                    }
                }
                t if t.filter_states().is_some() && layout.tag(i) => {
                    // `FilterUpgradeWidgetBase.renderBg` with the tag selector.
                    blit(batch, TABS, x, y, [66.0, 149.0, 87.0, 103.0]);
                    let (adding, list) = shown_tags(i);
                    for (n, tag) in list.iter().take(4).enumerate() {
                        let ty = y + 44.0 + 13.0 * n as f32;
                        blit(batch, TABS, x + 7.0, ty, [153.0, 149.0, 73.0, 13.0]);
                        game.native_text(batch, &trim_to(game, tag, 70.0), [ox + x + 9.0, oy + ty + 3.0], 0x00FF_FFFF, false);
                    }
                    for k in 0..2 {
                        let state = get(5 * i + k).max(0) as usize;
                        if let Some(&[u, v]) = filter_icons(tab, k).get(state) {
                            blit(batch, ICONS, x + 6.0 + 18.0 * k as f32, y + 22.0, [u, v, 18.0, 18.0]);
                        }
                        if inside([x + 6.0 + 18.0 * k as f32, y + 22.0, 18.0, 18.0], mouse) {
                            blit(batch, ICONS, x + 6.0 + 18.0 * k as f32, y + 22.0, [24.0, 18.0, 18.0, 18.0]);
                        }
                    }
                    // The tag icon where the components button was.
                    blit(batch, ICONS, x + 42.0, y + 22.0, [186.0, if adding { 36.0 } else { 0.0 }, 18.0, 18.0]);
                }
                t if t.filter_states().is_some() => {
                    blit(batch, TABS, x, y, [137.0, 0.0, w, h]);
                    for [sx, sy] in tab.slots() {
                        blit(batch, TABS, x + sx - 1.0, y + sy - 1.0, [233.0, 0.0, 18.0, 18.0]);
                    }
                    for k in 0..3 {
                        let state = get(5 * i + k).max(0) as usize;
                        if let Some(&[u, v]) = filter_icons(tab, k).get(state) {
                            blit(batch, ICONS, x + 6.0 + 18.0 * k as f32, y + 22.0, [u, v, 18.0, 18.0]);
                        }
                        if inside([x + 6.0 + 18.0 * k as f32, y + 22.0, 18.0, 18.0], mouse) {
                            blit(batch, ICONS, x + 6.0 + 18.0 * k as f32, y + 22.0, [24.0, 18.0, 18.0, 18.0]);
                        }
                    }
                }
                tb::Tab::Refill => {
                    // A filter's tab: its texture's top and bottom halves, stretched.
                    let (top, bottom) = ((h / 2.0).floor(), h - (h / 2.0).floor());
                    blit(batch, TABS, x, y, [137.0, 0.0, w, top]);
                    blit(batch, TABS, x, y + top, [137.0, 103.0 - bottom, w, bottom]);
                    for [sx, sy] in tab.slots() {
                        blit(batch, TABS, x + sx - 1.0, y + sy - 1.0, [233.0, 0.0, 18.0, 18.0]);
                    }
                }
                _ => {
                    let [u, v] = match tab {
                        tb::Tab::Tanks => [0.0, 0.0],
                        tb::Tab::Crafting => [51.0, 0.0],
                        tb::Tab::Jukebox => [137.0, 103.0],
                        _ => [0.0, 118.0],
                    };
                    blit(batch, TABS, x, y, [u, v, w, h]);
                }
            }
            if tab == tb::Tab::Plain {
                continue;
            }
            // The remove button (`renderRemoveButton`).
            blit(batch, ICONS, x + w - 21.0, y + 3.0, [42.0, 36.0, 18.0, 18.0]);
            let d = |k: usize| get(5 * i + k);
            if tab.cooking().is_some() {
                // `AbstractSmeltingWidget`: the flame burning down, the arrow filling.
                let [cook, total, burn, burn_total] = [d(0), d(1), d(2), d(3)];
                if burn > 0 && burn_total > 0 {
                    let k = (13.0 * burn as f32 / burn_total as f32).floor().clamp(0.0, 13.0);
                    blit(batch, TABS, x + 7.0, y + 55.0 - k, [0.0, 213.0 - k, 14.0, k + 1.0]);
                }
                if cook > 0 && total > 0 {
                    let l = (24.0 * cook as f32 / total as f32).floor().clamp(0.0, 24.0);
                    blit(batch, TABS, x + 28.0, y + 42.0, [14.0, 200.0, l, 13.0]);
                }
            }
            if tab == tb::Tab::Jukebox {
                for (k, bx) in [24.0, 42.0].into_iter().enumerate() {
                    if inside([x + bx, y + 22.0, 18.0, 18.0], mouse) {
                        blit(batch, ICONS, x + bx, y + 22.0, [24.0, 18.0, 18.0, 18.0]);
                    }
                    let _ = k;
                }
            }
        }
    }

    // The settings tab (`SettingsWidget`), and the settings screen's tabs.
    let [sx, sy] = layout.settings_tab();
    blit(batch, ICONS, sx, sy, [0.0, 0.0, 24.0, 24.0]);
    blit(batch, ICONS, sx + 3.0, sy + 3.0, if settings { [42.0, 18.0, 18.0, 18.0] } else { [42.0, 0.0, 18.0, 18.0] });
    if settings {
        for (w, [x, y, tw, th]) in settings_tabs(&layout, opened, &live) {
            match w {
                Widget::Unsortables if tw > 24.0 => {
                    blit(batch, TABS, x, y, [0.0, 72.0, tw, th]);
                    blit(batch, ICONS, x + 3.0, y + 3.0, [60.0, 18.0, 18.0, 18.0]);
                    for (bx, v) in [(6.0, 18.0), (24.0, 36.0)] {
                        blit(batch, ICONS, x + bx, y + 22.0, [132.0, v, 18.0, 18.0]);
                        if inside([x + bx, y + 22.0, 18.0, 18.0], mouse) {
                            blit(batch, ICONS, x + bx, y + 22.0, [24.0, 18.0, 18.0, 18.0]);
                        }
                    }
                }
                Widget::Memory if tw > 24.0 => {
                    blit(batch, TABS, x, y, [203.0, 0.0, tw, th]);
                    blit(batch, ICONS, x + 3.0, y + 3.0, [78.0, 18.0, 18.0, 18.0]);
                    let u = if live.matching { 96.0 } else { 114.0 };
                    blit(batch, ICONS, x + 6.0, y + 22.0, [u, 36.0, 18.0, 18.0]);
                }
                _ => {
                    blit(batch, ICONS, x, y, [0.0, 0.0, 24.0, 24.0]);
                    let icon = match w {
                        Widget::Unsortables => [60.0, 0.0],
                        Widget::Memory => [78.0, 0.0],
                        _ if live.visible => [60.0, 36.0],
                        _ => [78.0, 36.0],
                    };
                    blit(batch, ICONS, x + 3.0, y + 3.0, [icon[0], icon[1], 18.0, 18.0]);
                }
            }
        }
    }

    // The tool slots' panel left of the screen (`renderToolsAddition`).
    if !settings && layout.info.tools {
        let n = layout.tier.tools as f32;
        blit(batch, ICONS, -20.0, 12.0, [0.0, 67.0, 23.0, 5.0]);
        for i in 0..layout.tier.tools {
            blit(batch, ICONS, -20.0, 17.0 + 18.0 * i as f32, [0.0, 72.0, 23.0, 18.0]);
        }
        blit(batch, ICONS, -20.0, 17.0 + 18.0 * n, [0.0, 90.0, 23.0, 5.0]);
    }

    // The panel (`renderInventoryBackground`): its top and its bottom, the
    // storage's height between them.
    let (texture, tw) = if layout.tier.extended() { (BACKGROUND_11, 212.0) } else { (BACKGROUND_9, 176.0) };
    let half = (layout.slots_height() / 2.0).floor();
    let x0 = layout.tank_offset();
    blit(batch, texture, x0, 0.0, [0.0, 0.0, tw, TOP_BAR + half]);
    blit(batch, texture, x0, TOP_BAR + half, [0.0, 256.0 - (98.0 + half), tw, 98.0 + half]);

    // The tanks' columns (`renderScreen`) and the tanks (`TankWidget`).
    if layout.tanks() {
        let (u, right) = if layout.tier.extended() { (0.0, 229.0) } else { (56.0, 193.0) };
        let top = TOP_BAR - 9.0 + half;
        let rest = 90.0 + half + TOP_BAR - 9.0;
        let v = 256.0 - (90.0 + half + TOP_BAR);
        blit(batch, TANKS_TEXTURE, 0.0, 0.0, [0.0, 0.0, 27.0, top]);
        blit(batch, TANKS_TEXTURE, 0.0, top, [u, v, 27.0, rest]);
        blit(batch, TANKS_TEXTURE, right, 0.0, [u + 28.0, 0.0, 27.0, top]);
        blit(batch, TANKS_TEXTURE, right, top, [u + 28.0, v, 27.0, rest]);
        if let Some(i) = layout.kinds.iter().position(|&t| t == tb::Tab::Tanks) {
            let capacity = (get(5 * i + 4) & 0xFFFF).max(1);
            for (side, [x, y, w, h]) in layout.tank_columns().into_iter().enumerate() {
                let (fluid, amount) = (get(5 * i + 2 * side), get(5 * i + 2 * side + 1) & 0xFFFF);
                let inner = h - 2.0;
                if let Some(&(_, _, own)) = usize::try_from(fluid).ok().and_then(|f| f.checked_sub(1)).and_then(|f| FLUIDS.get(f)) {
                    // A potion in its own colour, from the data slots after the trash bin's.
                    let at = 5 * layout.tier.upgrades + 11 + 4 * layout.tier.storage.div_ceil(16) + 2 * side;
                    let rgb = ((get(at) & 0xFF) << 16 | get(at + 1) & 0xFFFF) as u32;
                    let colour = if fluid == 4 && data.len() > at + 1 { 0xFF00_0000 | rgb } else { own };
                    let level = (inner * amount.clamp(0, capacity) as f32 / capacity as f32).round();
                    let (fx, fy) = (x + 1.0, y + 1.0 + inner - level);
                    if fluid == 2 {
                        // Lava's own texture, its first frame tiled up.
                        let mut drawn = 0.0;
                        while drawn < level {
                            let part = (level - drawn).min(16.0);
                            if !game.native_sprite(batch, "block/lava_still", [0.0, 16.0 - part, 16.0, part], [ox + fx, oy + fy + level - drawn - part, 16.0, part]) {
                                game.native_fill(batch, [ox + fx, oy + fy, 16.0, level], colour);
                                break;
                            }
                            drawn += part;
                        }
                    } else {
                        game.native_fill(batch, [ox + fx, oy + fy, 16.0, level], colour);
                    }
                }
                // The glass over it: top, middle and bottom.
                let rows = layout.visible;
                blit(batch, ICONS, x, y, [0.0, 95.0, 18.0, 18.0]);
                for r in 1..rows.saturating_sub(1) {
                    blit(batch, ICONS, x, y + 18.0 * r as f32, [0.0, 113.0, 18.0, 18.0]);
                }
                blit(batch, ICONS, x, y + 18.0 * (rows.max(1) - 1) as f32, [0.0, 131.0, 18.0, 18.0]);
                let _ = w;
            }
        }
    }

    // The storage's slots (`renderSlots`): full rows, then the last one's start.
    let per_row = layout.per_row();
    let storage = layout.tier.storage;
    let sx0 = if layout.tanks() { 29.0 } else { 7.0 };
    for r in 0..layout.visible {
        let in_row = storage.saturating_sub((r + layout.scroll) * per_row).min(per_row);
        if in_row > 0 {
            blit(batch, SLOTS_TEXTURE, sx0, TOP_BAR + 18.0 * r as f32, [0.0, 0.0, 18.0 * in_row as f32, 18.0]);
        }
    }
    if let Some([bx, by, bw, bh]) = layout.scroll_bar() {
        let rows = layout.tier.rows() as f32;
        let thumb = (bh * layout.visible as f32 / rows).max(8.0);
        let top = by + (bh - thumb) * layout.scroll as f32 / (rows - layout.visible as f32).max(1.0);
        game.native_fill(batch, [ox + bx, oy + by, bw, bh], 0xFF00_0000);
        game.native_fill(batch, [ox + bx, oy + top, bw, thumb], 0xFF80_8080);
        game.native_fill(batch, [ox + bx, oy + top, bw - 1.0, thumb - 1.0], 0xFFC0_C0C0);
    }

    if !settings {
        // `SortingButtons`: the four, the one under the cursor lit.
        let [bx, by] = layout.sorting();
        blit(batch, ICONS, bx, by, [77.0, 54.0, 50.0, 13.0]);
        for i in 0..4u8 {
            let r = [bx + 2.0 + 12.0 * f32::from(i), by + 3.0, 10.0, 10.0];
            if inside(r, mouse) {
                blit(batch, ICONS, bx + 1.0 + 12.0 * f32::from(i), by + 2.0, [78.0 + 12.0 * f32::from(i), 69.0, 12.0, 12.0]);
            }
        }
        // `ToolSlotsWidget`.
        let [tx, ty] = layout.tools_button();
        if inside([tx, ty, 10.0, 10.0], mouse) {
            blit(batch, ICONS, tx - 1.0, ty - 1.0, [78.0, 82.0, 12.0, 12.0]);
        }
        blit(batch, ICONS, tx, ty, if live.tools { [4.0, 34.0, 10.0, 10.0] } else { [4.0, 24.0, 10.0, 10.0] });
        // The bar's buttons (`Button.drawButton`).
        for (w, [x, y, bw, bh]) in widgets(menu, data) {
            let icon = match w {
                Widget::Equip => [63.0, 56.0],
                Widget::Unequip => [63.0, 67.0],
                Widget::SleepingBag => [91.0, 83.0],
                Widget::More => [4.0, 44.0],
                Widget::Ability => [44.0, if live.ability_on { 56.0 } else { 67.0 }],
                _ => continue,
            };
            blit(batch, ICONS, x + 1.0, y + 1.0, [icon[0], icon[1], bw - 2.0, bh - 2.0]);
            if inside([x, y, bw, bh], mouse) {
                blit(batch, ICONS, x, y, [78.0, 82.0, bw, bh]);
            }
        }
    }

    // The unsortable slots' mark, the kept slots' items (`drawUnsortableSlots`,
    // `drawMemorySlots`); on the settings screen with its tab open, the kept
    // slots' marks.
    for s in (0..storage).filter(|&s| layout.storage_shown(s)) {
        let [x, y] = layout.storage_slot(s);
        if live.unsortable.get(s).copied().unwrap_or(false) {
            blit(batch, ICONS, x, y, [25.0, 55.0, 16.0, 16.0]);
        }
        if live.memory.get(s).copied().unwrap_or(false) {
            if settings && live.tab == 2 {
                let v = if live.matched.get(s).copied().unwrap_or(false) { 73.0 } else { 91.0 };
                blit(batch, ICONS, x, y, [25.0, v, 16.0, 16.0]);
            }
            if let Some((_, item)) = layout.info.memory.iter().find(|m| m.0 == s) {
                game.native_item(batch, item, [ox + x, oy + y], 16.0);
                game.native_fill(batch, [ox + x, oy + y, 16.0, 16.0], 0x30FF_FFFF);
            }
        }
    }

    // The labels (`titleLabelX`, `inventoryLabelX`): grey, no shadow.
    let title = game.native_screen_title().unwrap_or_else(|| tr(game, "screen.travelersbackpack.title"));
    game.native_text(batch, &title, [ox + 8.0 + layout.tank_offset(), oy + 6.0], 0x0040_4040, false);
    let inventory = tr(game, "container.inventory");
    game.native_text(batch, &inventory, [ox + 8.0 + layout.additions(), oy + 3.0 + TOP_BAR + layout.slots_height()], 0x0040_4040, false);
}

// ---- picking up (`ItemEntityMixin.playerTouch`) -------------------------------------------

/// A wearer touches a stack: with the pickup upgrade, the backpack takes it
/// first (or the void upgrade destroys it), so the player's own pickup waits
/// and the upgrade's tick takes it in.
pub(super) fn item_picked_up(game: &Game<'_>, player: u128, stack: &ItemStack, position: [f64; 3], canceled: &mut bool) {
    if *canceled || worn_menu_open(player) {
        return;
    }
    let Some(backpack) = worn(game, player) else {
        return;
    };
    let tier = stack_tier(&backpack);
    let mut items = contents(&backpack, tier.slots());
    let upgrades = upgrades_in(&items, tier);
    if !upgrades.iter().any(|u| u == "travelersbackpack:pickup_upgrade") {
        return;
    }
    if !fits_storage(game, stack) {
        return;
    }
    if filter_matches(game, &items, tier, tb::Tab::Pickup, stack) == Some(false) {
        return;
    }
    let voided = voids(game, &items, tier, stack);
    let fits = insert_stacked(game, &mut items[..tier.storage], stack.clone()).is_none_or(|left| left.count < stack.count);
    if voided || fits {
        *canceled = true;
        with(|s| s.touched.push((player, position)));
    }
}

// ---- the tanks on the model (`BackpackDynamicModel`'s fluid) ----------------------------------

/// What a placed backpack's client copy gets: each tank's fluid number and
/// fill in thousandths, `[left, left fill, right, right fill]`.
const FLUIDS_TAG: &str = "Fluids";

/// A backpack's tanks for the model, as `[left, left fill, right, right fill]`.
fn tank_tag(items: &[Option<ItemStack>], tier: tb::Tier) -> Option<Vec<i32>> {
    tank_levels(items, tier).map(|t| vec![t[0].0, (t[0].1 * 1000.0) as i32, t[1].0, (t[1].1 * 1000.0) as i32])
}

/// What clients get of a placed backpack (`getUpdateTag`): its tanks' fluids.
pub(super) fn block_entity_update_tag(entity: &horizon_client::containers::ModBlockEntity, tag: &mut Option<Compound>) {
    if entity.id != tb::BLOCK_ENTITY {
        return;
    }
    let tier = placed_tier(&entity.nbt, &entity.items);
    let mut c = Compound::new();
    if let Some(levels) = tank_tag(&entity.items, tier) {
        c.insert(FLUIDS_TAG.into(), Tag::IntArray(levels));
    }
    // Its dye, for the body drawn in it.
    set_block_dye(&mut c, block_dye(&entity.nbt));
    *tag = Some(c);
}

/// A placed backpack's tanks changed: its clients are told (`sendBlockUpdated`).
fn sync_fluids(game: &mut Game<'_>, pos: [i32; 3], entity: &mut horizon_client::containers::ModBlockEntity) {
    let tier = placed_tier(&entity.nbt, &entity.items);
    let now = tank_tag(&entity.items, tier).map(Tag::IntArray);
    let key = horizon_world::nbt::JavaString::from("FluidsSent");
    if entity.nbt.get(&key) != now.as_ref() {
        match now {
            Some(t) => {
                entity.nbt.insert(key, t);
            }
            None => {
                entity.nbt.remove(&key);
            }
        }
        game.native_block_entity_sync(pos);
    }
}

/// Each tank's inside, in the model's texels (its north turn): within the
/// glass and pillars, over the bottom.
const TANK_INSIDE: [([f32; 3], [f32; 3]); 2] = [([1.8, 0.8, 6.3], [3.3, 7.0, 7.8]), ([12.7, 0.8, 6.3], [14.2, 7.0, 7.8])];

/// Draws the fluid in the tanks, `levels` each tank's fluid number and fill,
/// the model turned `turn` degrees as its block state turns it.
fn draw_tank_fluids(game: &Game<'_>, draw: &mut super::native_block_entity_draw::BlockEntityDraw, levels: [(i32, f32); 2], turn: f32) {
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], -turn, [0.5, 0.5, 0.5]);
    for ((code, fill), (from, to)) in levels.into_iter().zip(TANK_INSIDE) {
        if code <= 0 || fill <= 0.0 {
            continue;
        }
        let (sprite, tint) = match code {
            1 => ("block/water_still", 0x3F76E4),
            2 => ("block/lava_still", 0xFFFFFF),
            3 => ("block/white_concrete", 0xFFFFFF),
            c => ("block/water_still", tank_code_colour(c)),
        };
        let top = from[1] + (to[1] - from[1]) * fill.min(1.0);
        let face = Some(super::native_block_entity_draw::CubeFace { sprite, uv: None });
        game.native_draw_cube(draw, from, [to[0], top, to[2]], [face, face, face, face, face, face], tint);
    }
    draw.pop();
}

/// The model's turn for a facing (the blockstates' `y`).
fn model_turn(facing: &str) -> f32 {
    match facing {
        "west" => 270.0,
        "east" => 90.0,
        "south" => 180.0,
        _ => 0.0,
    }
}

/// A placed backpack drawn: its tanks' fluid, as the server sent it.
pub(super) fn block_entity_submit(
    game: &Game<'_>,
    state: horizon_world::registry::BlockStateId,
    entity: &horizon_client::containers::ModBlockEntity,
    draw: &mut super::native_block_entity_draw::BlockEntityDraw,
) {
    if entity.id != tb::BLOCK_ENTITY {
        return;
    }
    if let Some(colour) = block_dye(&entity.nbt)
        && game.native_block_state_property(state, "dyed") == Some("true")
    {
        let facing = game.native_block_state_property(state, "facing").unwrap_or("north").to_owned();
        draw_dyed_body(game, draw, colour, model_turn(&facing));
    }
    // The world's own copy (one player) has the items; a joined player's,
    // the tag the server sent.
    let tier = placed_tier(&entity.nbt, &entity.items);
    let t = match tank_tag(&entity.items, tier) {
        Some(t) => t,
        None => match entity.nbt.get(&horizon_world::nbt::JavaString::from(FLUIDS_TAG)) {
            Some(Tag::IntArray(t)) => t.clone(),
            _ => return,
        },
    };
    if t.len() < 4 {
        return;
    }
    let levels = [(t[0], t[1] as f32 / 1000.0), (t[2], t[3] as f32 / 1000.0)];
    let facing = game.native_block_state_property(state, "facing").unwrap_or("north").to_owned();
    draw_tank_fluids(game, draw, levels, model_turn(&facing));
}

// ---- the backpack's settings (`SORT_TYPE`, `SHOW_TOOL_SLOTS`, `SHOW_MORE_BUTTONS`, `SLOTS`, `IS_VISIBLE`) ----

/// Where a backpack item keeps its settings: its `minecraft:custom_data`,
/// under this key; a placed one, in its block entity's compound.
const SETTINGS_KEY: &str = "TravelersBackpackSettings";
const SETTINGS_NBT: &str = "Settings";

/// `BackpackWrapper`'s settings: the sort (`SortSelector.SortType`: category,
/// name, count), whether the tool slots and the more buttons show, the slots
/// sorting leaves alone (`unsortables`), the slots kept for an item
/// (`memory`: the slot, the item, whether its components must match), and
/// whether the worn backpack shows on the back.
#[derive(Clone, Debug, Default, PartialEq)]
struct Settings {
    sort: i32,
    tools: bool,
    more: bool,
    unsortable: Vec<usize>,
    memory: Vec<(usize, ItemStack, bool)>,
    hidden: bool,
    /// The ability switched off (`ABILITY_ENABLED` false).
    ability_off: bool,
    /// Ticks until the ability is ready again (`COOLDOWN`).
    cooldown: i32,
}

impl Settings {
    fn of(tag: Option<&Tag>) -> Self {
        let Some(Tag::Compound(c)) = tag else {
            return Self { ability_off: !cfg().force_ability_enabled, ..Self::default() };
        };
        let get = |k: &str| c.get(&horizon_world::nbt::JavaString::from(k));
        let int = |k: &str| match get(k) {
            Some(Tag::Int(n)) => *n,
            Some(Tag::Byte(n)) => i32::from(*n),
            _ => 0,
        };
        let unsortable = match get("Unsortable") {
            Some(Tag::IntArray(v)) => v.iter().filter_map(|&n| usize::try_from(n).ok()).collect(),
            _ => Vec::new(),
        };
        let mut memory = Vec::new();
        if let Some(Tag::List(list)) = get("Memory") {
            for t in list.iter() {
                let Tag::Compound(m) = t else {
                    continue;
                };
                let g = |k: &str| m.get(&horizon_world::nbt::JavaString::from(k));
                let slot = match g("Slot") {
                    Some(Tag::Int(n)) => usize::try_from(*n).ok(),
                    _ => None,
                };
                let item = g("Item").and_then(horizon_items::item_nbt::item_from_nbt);
                let matching = matches!(g("Match"), Some(Tag::Byte(1)));
                if let (Some(slot), Some(item)) = (slot, item) {
                    memory.push((slot, item, matching));
                }
            }
        }
        Self {
            sort: int("Sort").rem_euclid(3),
            tools: int("Tools") != 0,
            more: int("More") != 0,
            unsortable,
            memory,
            hidden: int("Hidden") != 0,
            ability_off: if get("AbilityOff").is_some() { int("AbilityOff") != 0 } else { !cfg().force_ability_enabled },
            cooldown: int("Cooldown").max(0),
        }
    }

    fn tag(&self) -> Option<Tag> {
        if *self == Self::default() && cfg().force_ability_enabled {
            return None;
        }
        let mut c = Compound::new();
        c.insert("Sort".into(), Tag::Int(self.sort));
        c.insert("Tools".into(), Tag::Byte(i8::from(self.tools)));
        c.insert("More".into(), Tag::Byte(i8::from(self.more)));
        c.insert("Hidden".into(), Tag::Byte(i8::from(self.hidden)));
        c.insert("AbilityOff".into(), Tag::Byte(i8::from(self.ability_off)));
        c.insert("Cooldown".into(), Tag::Int(self.cooldown));
        c.insert("Unsortable".into(), Tag::IntArray(self.unsortable.iter().map(|&n| n as i32).collect()));
        let memory = self
            .memory
            .iter()
            .map(|(slot, item, matching)| {
                let mut m = Compound::new();
                m.insert("Slot".into(), Tag::Int(*slot as i32));
                m.insert("Item".into(), horizon_items::item_nbt::item_to_nbt(item));
                m.insert("Match".into(), Tag::Byte(i8::from(*matching)));
                Tag::Compound(m)
            })
            .collect();
        c.insert("Memory".into(), Tag::List(memory));
        Some(Tag::Compound(c))
    }

    /// `getMemorizedSlot`.
    fn memorized(&self, slot: usize) -> Option<&(usize, ItemStack, bool)> {
        self.memory.iter().find(|m| m.0 == slot)
    }
}

fn item_settings(stack: &ItemStack) -> Settings {
    Settings::of(custom_data(stack).get(&horizon_world::nbt::JavaString::from(SETTINGS_KEY)))
}

fn set_item_settings(stack: &mut ItemStack, settings: &Settings) {
    let mut data = custom_data(stack);
    let key = horizon_world::nbt::JavaString::from(SETTINGS_KEY);
    match settings.tag() {
        Some(t) => {
            data.insert(key, t);
        }
        None => {
            data.remove(&key);
        }
    }
    set_custom_data(stack, data);
}

fn block_settings(nbt: &Compound) -> Settings {
    Settings::of(nbt.get(&horizon_world::nbt::JavaString::from(SETTINGS_NBT)))
}

fn set_block_settings(nbt: &mut Compound, settings: &Settings) {
    let key = horizon_world::nbt::JavaString::from(SETTINGS_NBT);
    match settings.tag() {
        Some(t) => {
            nbt.insert(key, t);
        }
        None => {
            nbt.remove(&key);
        }
    }
}

/// The settings of the backpack `uuid` has open from `source`.
fn source_settings(game: &Game<'_>, uuid: u128, source: Source) -> Settings {
    match source {
        Source::Hand(slot) => game
            .lan
            .as_ref()
            .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid))
            .and_then(|g| g.inventory.get(slot).cloned().flatten())
            .map(|s| item_settings(&s))
            .unwrap_or_default(),
        Source::Worn => worn(game, uuid).map(|s| item_settings(&s)).unwrap_or_default(),
        Source::Other(owner) => worn(game, owner).map(|s| item_settings(&s)).unwrap_or_default(),
        Source::Block(pos) => game.native_mod_block_entity(pos).map(|m| block_settings(&m.nbt)).unwrap_or_default(),
    }
}

/// What a shift-click into the storage keeps to.
fn move_rules(settings: &Settings, tier: tb::Tier) -> horizon_client::lan::containers::travelersbackpack_menu::MoveRules {
    horizon_client::lan::containers::travelersbackpack_menu::MoveRules {
        storage: tier.storage,
        unsortable: settings.unsortable.clone(),
        memory: settings.memory.clone(),
    }
}

fn set_source_settings(game: &mut Game<'_>, uuid: u128, source: Source, settings: &Settings) {
    if let (Some((_, _, window)), Some(tier)) = (with(|s| s.open.get(&uuid).cloned()), with(|s| s.extras.get(&uuid).map(|e| e.0)))
        && let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn)
    {
        horizon_client::lan::containers::travelersbackpack_menu::set_rules(conn, window, move_rules(settings, tier));
    }
    match source {
        Source::Hand(slot) => {
            if let Some(stack) = game
                .lan
                .as_mut()
                .and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid))
                .and_then(|g| g.inventory.get_mut(slot))
                .and_then(Option::as_mut)
                .filter(|s| tb::is_backpack(&s.item))
            {
                set_item_settings(stack, settings);
            }
        }
        Source::Worn => {
            if let Some(mut stack) = worn(game, uuid) {
                // The cooldown ran on while the screen was open: it stays as it is now.
                let settings = &Settings { cooldown: item_settings(&stack).cooldown, ..settings.clone() };
                set_item_settings(&mut stack, settings);
                set_worn(game, uuid, Some(&stack));
                tell_worn(game, uuid);
            }
        }
        Source::Other(owner) => {
            if let Some(mut stack) = worn(game, owner) {
                let settings = &Settings { cooldown: item_settings(&stack).cooldown, ..settings.clone() };
                set_item_settings(&mut stack, settings);
                set_worn(game, owner, Some(&stack));
                tell_worn(game, owner);
            }
        }
        Source::Block(pos) => {
            let settings = settings.clone();
            game.native_with_mod_block_entity(pos, |m| set_block_settings(&mut m.nbt, &settings));
        }
    }
}

/// The settings screen's state for a player: its open tab (1 the
/// unsortables, 2 the memory) and whether the memory matches components.
#[derive(Clone, Copy, Debug, Default)]
struct SettingsUi {
    tab: u8,
    matching: bool,
}

/// What the screen's data slots carry past the tabs': the sort, the flags
/// (tool slots shown 1, more buttons 2, the settings screen 4, its tab
/// 8 and 16, components matched 32, shown on the back 64), then one bit a
/// storage slot, 16 to a data slot: the unsortables, the memory, its matches.
fn settings_data(settings: &Settings, ui: Option<SettingsUi>, storage: usize) -> Vec<i32> {
    let mut flags = i32::from(settings.tools) | i32::from(settings.more) << 1 | i32::from(!settings.hidden) << 6;
    if let Some(ui) = ui {
        flags |= 4 | i32::from(ui.tab & 3) << 3 | i32::from(ui.matching) << 5;
    }
    let words = storage.div_ceil(16);
    let mask = |slots: &mut dyn Iterator<Item = usize>| {
        let mut m = vec![0i32; words];
        for s in slots {
            if s < storage {
                m[s / 16] |= 1 << (s % 16);
            }
        }
        m
    };
    let mut data = vec![settings.sort, flags];
    data.extend(mask(&mut settings.unsortable.iter().copied()));
    data.extend(mask(&mut settings.memory.iter().map(|m| m.0)));
    data.extend(mask(&mut settings.memory.iter().filter(|m| m.2).map(|m| m.0)));
    data
}

/// The menu's data slots for `uuid`'s open backpack: its tabs', then its settings'.
fn full_data(game: &Game<'_>, uuid: u128, items: &[Option<ItemStack>], tier: tb::Tier) -> Vec<i32> {
    let mut data = menu_data(items, tier);
    let source = with(|s| s.open.get(&uuid).map(|o| o.0));
    let settings = source.map(|source| source_settings(game, uuid, source)).unwrap_or_default();
    let ui = with(|s| s.settings.get(&uuid).copied());
    data.extend(settings_data(&settings, ui, tier.storage));
    // Which upgrade slots hold one.
    data.extend(upgrade_masks(items, tier));
    data.extend(ability_words(game, uuid, source, &settings));
    data.extend(trash_words(uuid, tier.storage, tier.menu_slots()));
    data.extend(tank_colour_words(items, tier));
    data
}

/// Which upgrade slots hold one, which are on, which can be switched off.
fn upgrade_masks(items: &[Option<ItemStack>], tier: tb::Tier) -> [i32; 3] {
    let up = |i: usize| items.get(tier.storage + i).cloned().flatten();
    let mask = |f: &dyn Fn(&ItemStack) -> bool| (0..tier.upgrades).filter(|&i| up(i).is_some_and(|u| f(&u))).fold(0, |m, i| m | 1 << i);
    [mask(&|_| true), mask(&|u| upgrade_enabled(u)), mask(&|u| can_switch_off(&u.item))]
}

/// Sends `uuid`'s open menu its data slots again.
fn resend_data(game: &mut Game<'_>, uuid: u128) {
    let Some((_, _, window)) = with(|s| s.open.get(&uuid).cloned()) else {
        return;
    };
    let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
        return;
    };
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    let Some((items, _)) =
        game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
    else {
        return;
    };
    let data = if with(|s| s.settings.contains_key(&uuid)) {
        let mut d = vec![0; 5 * tier.upgrades];
        let source = with(|s| s.open.get(&uuid).map(|o| o.0));
        let settings = source.map(|source| source_settings(game, uuid, source)).unwrap_or_default();
        d.extend(settings_data(&settings, with(|s| s.settings.get(&uuid).copied()), tier.storage));
        d
    } else if items.len() >= tier.menu_slots() {
        full_data(game, uuid, &items, tier)
    } else {
        return;
    };
    if let Some(host) = game.lan.as_mut() {
        horizon_client::lan::containers::travelersbackpack_menu::send_data(&mut host.hub, conn, window, &data);
    }
}

// ---- sorting and moving (`ContainerSorter`, `SortSelector`) ------------------------------------

/// `SortSelector.specialCases`: the count inverted, the enchantments, the damage.
fn sort_suffix(stack: &ItemStack) -> String {
    let mut s = format!("{:04}", 9999 - stack.count.clamp(0, 9999));
    if let Some(Tag::Compound(e)) = stack.data().and_then(|d| d.other("minecraft:enchantments").or(d.other("minecraft:stored_enchantments"))) {
        let mut names: Vec<String> = e.keys().filter_map(|k| k.to_string().ok()).collect();
        names.sort();
        s.push_str(&format!(" {} {}", names.len(), names.join(" ")));
    }
    if stack.damage != 0 {
        s.push_str(&stack.damage.to_string());
    }
    s
}

/// `ContainerSorter.canMergeItems`/`combineStacks`: `new` onto the stacks
/// already collected, from the last, then kept (`addStackWithMerge`).
fn add_with_merge(game: &Game<'_>, stacks: &mut Vec<ItemStack>, mut new: ItemStack) {
    let max = max_stack(game, &new);
    if max > 1 && new.count < max {
        let mut j = stacks.len();
        while j > 0 {
            j -= 1;
            let old = &mut stacks[j];
            if old.count >= max || !same_item(old, &new) {
                continue;
            }
            let moved = (max - new.count).min(old.count);
            new.count += moved;
            old.count -= moved;
            if old.count <= 0 {
                stacks.remove(j);
            }
            if new.count >= max {
                break;
            }
        }
    }
    stacks.push(new);
}

fn max_stack(game: &Game<'_>, stack: &ItemStack) -> u32 {
    game.items().get(&stack.item).map_or(64, |i| i.max_stack.max(1))
}

/// `sortBackpack`: the storage's stacks, merged and in the sort's order,
/// back into its slots, the unsortables left as they are.
fn sort_storage(game: &Game<'_>, storage: &mut [Option<ItemStack>], settings: &Settings) {
    let mut stacks = Vec::new();
    for (i, slot) in storage.iter().enumerate() {
        if let Some(s) = slot.clone().filter(|_| !settings.unsortable.contains(&i)) {
            add_with_merge(game, &mut stacks, s);
        }
    }
    if stacks.is_empty() {
        return;
    }
    match settings.sort {
        // By item name: the id's path.
        1 => stacks.sort_by_key(|s| format!("{}{}", s.item.split_once(':').map_or(&*s.item, |p| p.1), sort_suffix(s))),
        // By count: the items most of which there are first.
        2 => {
            let mut totals: Vec<(String, u32)> = Vec::new();
            for s in &stacks {
                match totals.iter_mut().find(|t| t.0 == *s.item) {
                    Some(t) => t.1 += s.count,
                    None => totals.push((s.item.to_string(), s.count)),
                }
            }
            totals.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            stacks.sort_by_key(|s| totals.iter().position(|t| t.0 == *s.item).unwrap_or(usize::MAX));
        }
        _ => stacks.sort_by_key(|s| format!("{}{}", s.item, sort_suffix(s))),
    }
    let mut next = stacks.into_iter();
    for (i, slot) in storage.iter_mut().enumerate() {
        if settings.unsortable.contains(&i) {
            continue;
        }
        *slot = next.next();
    }
}

/// `CustomWrapper.insertItem` into one slot: a kept slot takes only its
/// item, an unsortable none; else as an item handler does. What is left.
fn insert_into(game: &Game<'_>, storage: &mut [Option<ItemStack>], slot: usize, mut stack: ItemStack, settings: &Settings) -> Option<ItemStack> {
    if stack.count <= 0 {
        return None;
    }
    if let Some((_, item, matching)) = settings.memorized(slot) {
        let fits = if *matching { same_item(item, &stack) } else { item.item == stack.item };
        if !fits {
            return Some(stack);
        }
    } else if settings.unsortable.contains(&slot) {
        return Some(stack);
    }
    put_into(game, &mut storage[slot], &mut stack);
    (stack.count > 0).then_some(stack)
}

/// One slot takes what it can of `stack` (`ItemStackHandler.insertItem`).
fn put_into(game: &Game<'_>, slot: &mut Option<ItemStack>, stack: &mut ItemStack) {
    let max = max_stack(game, stack);
    match slot {
        None => {
            let mut put = stack.clone();
            put.count = stack.count.min(max);
            stack.count -= put.count;
            *slot = Some(put);
        }
        Some(s) if same_item(s, stack) && s.count < max => {
            let moved = (max - s.count).min(stack.count);
            s.count += moved;
            stack.count -= moved;
        }
        _ => {}
    }
}

/// The player inventory slots a move reads (`shift`: the hotbar too), the
/// open backpack's own slot left out.
fn player_range(shift: bool, skip: Option<usize>) -> impl Iterator<Item = usize> {
    (if shift { 0 } else { 9 }..36).filter(move |&i| Some(i) != skip)
}

/// `quickStackToBackpackNoSort`: what of the player's the storage holds some of goes in.
fn quick_stack(game: &Game<'_>, storage: &mut [Option<ItemStack>], inv: &mut [Option<ItemStack>], shift: bool, skip: Option<usize>, settings: &Settings) {
    for i in player_range(shift, skip) {
        let Some(held) = inv.get(i).cloned().flatten() else {
            continue;
        };
        if !storage.iter().flatten().any(|s| s.item == held.item) {
            continue;
        }
        let mut left = Some(held);
        for j in 0..storage.len() {
            let Some(l) = left.take() else {
                break;
            };
            left = insert_into(game, storage, j, l, settings);
        }
        inv[i] = left;
    }
}

/// `transferToBackpackNoSort`: the kept slots take their items first, then
/// everything goes where it fits.
fn to_backpack(game: &Game<'_>, storage: &mut [Option<ItemStack>], inv: &mut [Option<ItemStack>], shift: bool, skip: Option<usize>, settings: &Settings) {
    for (slot, item, matching) in settings.memory.clone() {
        for i in player_range(shift, skip) {
            let Some(held) = inv.get(i).cloned().flatten() else {
                continue;
            };
            let fits = if matching { same_item(&item, &held) } else { item.item == held.item };
            if fits && slot < storage.len() {
                inv[i] = insert_into(game, storage, slot, held, settings);
            }
        }
    }
    for i in player_range(shift, skip) {
        let Some(held) = inv.get(i).cloned().flatten() else {
            continue;
        };
        let mut left = Some(held);
        for j in 0..storage.len() {
            let Some(l) = left.take() else {
                break;
            };
            left = insert_into(game, storage, j, l, settings);
        }
        inv[i] = left;
    }
}

/// `transferToPlayer`: the storage, the unsortables but, into the inventory
/// above the hotbar.
fn to_player(game: &Game<'_>, storage: &mut [Option<ItemStack>], inv: &mut [Option<ItemStack>], settings: &Settings) {
    for i in 0..storage.len() {
        if settings.unsortable.contains(&i) {
            continue;
        }
        let Some(mut stack) = storage[i].take() else {
            continue;
        };
        for j in 9..36.min(inv.len()) {
            if stack.count <= 0 {
                break;
            }
            put_into(game, &mut inv[j], &mut stack);
        }
        storage[i] = (stack.count > 0).then_some(stack);
    }
}

/// A sort button (`ServerboundActionTagPacket.SORTER`) on `uuid`'s open
/// backpack: 0 sorts (with shift, the next way of sorting), 1 stacks onto
/// what it has, 2 moves the inventory in, 3 moves the storage out.
fn sort_button(game: &mut Game<'_>, uuid: u128, kind: u8, shift: bool) {
    let Some((source, _, _)) = with(|s| s.open.get(&uuid).cloned()) else {
        return;
    };
    if with(|s| s.settings.contains_key(&uuid)) {
        return;
    }
    let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
        return;
    };
    let mut settings = source_settings(game, uuid, source);
    if kind == 0 && shift {
        settings.sort = (settings.sort + 1) % 3;
        set_source_settings(game, uuid, source, &settings);
        resend_data(game, uuid);
        return;
    }
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    let Some((mut items, carried)) =
        game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
    else {
        return;
    };
    if items.len() < tier.storage {
        return;
    }
    let skip = match source {
        Source::Hand(slot) => Some(slot),
        _ => None,
    };
    let Some(mut inv) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.inventory.clone()) else {
        return;
    };
    {
        let storage = &mut items[..tier.storage];
        match kind {
            0 => sort_storage(game, storage, &settings),
            1 => quick_stack(game, storage, &mut inv, shift, skip, &settings),
            2 => to_backpack(game, storage, &mut inv, shift, skip, &settings),
            _ => to_player(game, storage, &mut inv, &settings),
        }
    }
    if let Some(host) = game.lan.as_mut() {
        if let Some(g) = host.hub.guests.iter_mut().find(|g| g.uuid == uuid) {
            g.inventory = inv;
        }
        horizon_client::lan::containers::travelersbackpack_menu::set_items(&mut host.hub, conn, items, carried);
    }
}

// ---- the settings screen (`BackpackSettingsScreen`, `BackpackSettingsMenu`) ------------------

/// A settings button: the tool slots or the more buttons shown or not (the
/// menu opens again for the tool slots), the settings screen opened or left,
/// its tabs, selecting all or no slots, matching components, showing the
/// worn backpack.
fn settings_button(game: &mut Game<'_>, uuid: u128, action: Action) {
    let Some((source, dimension, window)) = with(|s| s.open.get(&uuid).cloned()) else {
        return;
    };
    let in_settings = with(|s| s.settings.contains_key(&uuid));
    let mut settings = source_settings(game, uuid, source);
    let reopen = |game: &mut Game<'_>| {
        let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
            return;
        };
        if let Some(host) = game.lan.as_mut()
            && horizon_client::lan::containers::travelersbackpack_menu::close(&mut host.hub, conn, window)
        {
            with(|s| s.reopen.insert(uuid));
        }
    };
    match action {
        Action::ToggleTools if !in_settings => {
            settings.tools = !settings.tools;
            set_source_settings(game, uuid, source, &settings);
            reopen(game);
        }
        Action::ToggleMore if !in_settings => {
            settings.more = !settings.more;
            set_source_settings(game, uuid, source, &settings);
            resend_data(game, uuid);
        }
        Action::AbilitySlider if !in_settings && ability_words(game, uuid, Some(source), &settings)[0] & 1 != 0 => {
            settings.ability_off = !settings.ability_off;
            set_source_settings(game, uuid, source, &settings);
            resend_data(game, uuid);
        }
        Action::Settings(open) if open != in_settings => {
            // A placed backpack's settings, one player at a time (`getSettingsUser`).
            if open
                && let Source::Block(pos) = source
                && with(|s| s.open.iter().any(|(u, o)| *u != uuid && o.0 == Source::Block(pos) && s.settings.contains_key(u)))
            {
                return;
            }
            if open {
                with(|s| s.settings_enter.insert(uuid));
            }
            reopen(game);
        }
        Action::SettingsTab(tab) if in_settings => {
            with(|s| {
                if let Some(ui) = s.settings.get_mut(&uuid) {
                    ui.tab = if ui.tab == tab { 0 } else { tab };
                }
            });
            resend_data(game, uuid);
        }
        Action::SelectAll(all) if in_settings => {
            let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
                return;
            };
            settings.unsortable = if all { (0..tier.storage).collect() } else { Vec::new() };
            set_source_settings(game, uuid, source, &settings);
            resend_data(game, uuid);
        }
        Action::ToggleMatch if in_settings => {
            with(|s| {
                if let Some(ui) = s.settings.get_mut(&uuid) {
                    ui.matching = !ui.matching;
                }
            });
            resend_data(game, uuid);
        }
        Action::ToggleVisible if in_settings && source == Source::Worn => {
            settings.hidden = !settings.hidden;
            set_source_settings(game, uuid, source, &settings);
            resend_data(game, uuid);
        }
        _ => {}
    }
    let _ = dimension;
}

/// A click on a storage slot of the settings screen (`selectSlots`): with
/// its unsortables tab open, the left button marks the slot, the right one
/// unmarks it; with its memory tab, the left button keeps the slot for the
/// item in it, the right one forgets it. A drag (`QUICK_CRAFT`) does the same
/// over each slot.
fn settings_click(game: &mut Game<'_>, uuid: u128, slot: usize, button: i32, mode: i32) {
    let Some((source, _, _)) = with(|s| s.open.get(&uuid).cloned()) else {
        return;
    };
    let Some(ui) = with(|s| s.settings.get(&uuid).copied()) else {
        return;
    };
    let right = match mode {
        0 => button == 1,
        // A drag's slot (stage 1), its button in the high bits.
        5 if button & 3 == 1 => button >> 2 == 1,
        _ => return,
    };
    let mut settings = source_settings(game, uuid, source);
    match ui.tab {
        1 => {
            if right {
                settings.unsortable.retain(|&s| s != slot);
            } else if !settings.unsortable.contains(&slot) {
                settings.unsortable.push(slot);
                settings.unsortable.sort_unstable();
            }
        }
        2 => {
            if right {
                settings.memory.retain(|m| m.0 != slot);
            } else if settings.memorized(slot).is_none() {
                let item = game.lan.as_ref().and_then(|h| {
                    let conn = h.hub.guests.iter().find(|g| g.uuid == uuid)?.conn;
                    let (items, _) = horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn)?;
                    items.get(slot).cloned().flatten()
                });
                let Some(mut item) = item else {
                    return;
                };
                item.count = 1;
                settings.memory.push((slot, item, ui.matching));
                settings.memory.sort_by_key(|m| m.0);
            }
        }
        _ => return,
    }
    set_source_settings(game, uuid, source, &settings);
    resend_data(game, uuid);
}

/// The screen's title (`getBackpackScreenTitle`): the backpack's name, else "Backpack".
fn screen_title(name: Option<Tag>) -> Text {
    name.map(|n| horizon_client_core::component::name_text(&n)).unwrap_or_else(|| Text::translate("screen.travelersbackpack.title", &[]))
}

/// An open tab's remove button (`ServerActions.removeUpgrade`): the upgrade,
/// its tab's items in it, into the player's inventory; the menu opens again.
fn remove_upgrade(game: &mut Game<'_>, uuid: u128, window: i32, i: usize) {
    let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
        return;
    };
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    let Some((mut items, carried)) =
        game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
    else {
        return;
    };
    if items.len() < tier.menu_slots() || i >= tier.upgrades {
        return;
    }
    keep_tab_items(&mut items, tier);
    let tab = shown_tabs(&items, tier)[i];
    let Some(mut upgrade) = items[tier.storage + i].take() else {
        return;
    };
    empty_tab_block(game, uuid, &mut items, tier, i, tab);
    set_tab_open(&mut upgrade, false);
    with(|s| s.extras.insert(uuid, (tier, items[tier.storage..].to_vec())));
    if let Some(host) = game.lan.as_mut() {
        let hub = &mut host.hub;
        horizon_client::lan::containers::travelersbackpack_menu::set_items(hub, conn, items, carried);
        if horizon_client::lan::containers::travelersbackpack_menu::close(hub, conn, window) {
            with(|s| s.reopen.insert(uuid));
        }
    }
    give_back(game, uuid, upgrade);
}

// ---- the filters' buttons and switching upgrades off (`FilterSettingsBase`, `IEnable`) --------

/// Where an upgrade keeps its filter's buttons (`FILTER_SETTINGS`) and
/// whether it is on (`UPGRADE_ENABLED`): its `minecraft:custom_data`.
const FILTER_KEY: &str = "TravelersBackpackFilter";
const ENABLED_KEY: &str = "TravelersBackpackEnabled";

fn filter_settings(stack: &ItemStack) -> [i32; 3] {
    let tab = tb::Tab::of(Some(&stack.item));
    match custom_data(stack).get(&horizon_world::nbt::JavaString::from(FILTER_KEY)) {
        Some(Tag::IntArray(v)) if v.len() == 3 => [v[0], v[1], v[2]],
        _ => tab.filter_default(),
    }
}

fn set_filter_settings(stack: &mut ItemStack, settings: [i32; 3]) {
    let mut data = custom_data(stack);
    data.insert(FILTER_KEY.into(), Tag::IntArray(settings.to_vec()));
    set_custom_data(stack, data);
}

/// The upgrades that can be switched off (`IEnable`).
fn can_switch_off(item: &str) -> bool {
    [
        "void_upgrade",
        "pickup_upgrade",
        "magnet_upgrade",
        "feeding_upgrade",
        "refill_upgrade",
        "lantern_upgrade",
        "furnace_upgrade",
        "smoker_upgrade",
        "blast_furnace_upgrade",
    ]
    .iter()
    .any(|u| item.strip_prefix("travelersbackpack:") == Some(u))
}

fn upgrade_enabled(stack: &ItemStack) -> bool {
    !matches!(custom_data(stack).get(&horizon_world::nbt::JavaString::from(ENABLED_KEY)), Some(Tag::Byte(0)))
}

fn set_upgrade_enabled(stack: &mut ItemStack, on: bool) {
    let mut data = custom_data(stack);
    let key = horizon_world::nbt::JavaString::from(ENABLED_KEY);
    if on {
        data.remove(&key);
    } else {
        data.insert(key, Tag::Byte(0));
    }
    set_custom_data(stack, data);
}

/// A backpack's upgrade of kind `tab` that is on: its filter's items and buttons.
fn filter_upgrade(items: &[Option<ItemStack>], tier: tb::Tier, tab: tb::Tab) -> Option<(Vec<ItemStack>, [i32; 3])> {
    (0..tier.upgrades).find_map(|i| {
        let u = items.get(tier.storage + i)?.as_ref()?;
        (tb::Tab::of(Some(&u.item)) == tab && upgrade_enabled(u))
            .then(|| (tab_items(u).into_iter().take(9).flatten().collect(), filter_settings(u)))
    })
}

/// `FilterSettingsBase.compare`: by the item (its components too, unless
/// ignored), by its mod, or by a tag the two share.
fn filter_compare(game: &Game<'_>, settings: [i32; 3], filter: &ItemStack, stack: &ItemStack) -> bool {
    match settings[1] {
        1 => filter.item.split_once(':').map(|p| p.0) == stack.item.split_once(':').map(|p| p.0),
        2 => game.items().item_tags.values().any(|t| t.contains(&*filter.item) && t.contains(&*stack.item)),
        _ if settings[2] == 1 => filter.item == stack.item,
        _ => same_item(filter, stack),
    }
}

/// `matchesFilter` of a void, pickup or magnet filter: allowed, blocked, or
/// matching what the storage holds (`MATCH_CONTENTS`).
fn filter_matches(game: &Game<'_>, items: &[Option<ItemStack>], tier: tb::Tier, tab: tb::Tab, stack: &ItemStack) -> Option<bool> {
    let (filter, settings) = filter_upgrade(items, tier, tab)?;
    // The tag selector (`isTagFilter`): the item in any of the chosen tags.
    if has_tag_selector(tab) && settings[1] == 2 {
        let tags = (0..tier.upgrades)
            .find_map(|i| items.get(tier.storage + i)?.as_ref().filter(|u| tb::Tab::of(Some(&u.item)) == tab && upgrade_enabled(u)))
            .map(filter_tags)
            .unwrap_or_default();
        let tagged = tags.iter().any(|t| game.items().item_tags.get(t).is_some_and(|set| set.contains(&*stack.item)));
        return Some(if settings[0] == 1 { !tagged } else { tagged });
    }
    Some(match settings[0] {
        0 => filter.iter().any(|f| filter_compare(game, settings, f, stack)),
        1 => !filter.iter().any(|f| filter_compare(game, settings, f, stack)),
        _ => items[..tier.storage].iter().flatten().any(|f| filter_compare(game, settings, f, stack)),
    })
}

/// `VoidUpgrade.canVoid`.
fn voids(game: &Game<'_>, items: &[Option<ItemStack>], tier: tb::Tier, stack: &ItemStack) -> bool {
    filter_matches(game, items, tier, tb::Tab::Void, stack) == Some(true)
}

/// A filter button pressed: its next state (`FilterButton.nextState`).
fn filter_button(game: &mut Game<'_>, uuid: u128, i: usize, k: usize) {
    edit_upgrade(game, uuid, i, |u| {
        let Some(states) = tb::Tab::of(Some(&u.item)).filter_states() else {
            return;
        };
        let mut s = filter_settings(u);
        let was = s[1];
        s[k] = (s[k] + 1) % states[k];
        let tab = tb::Tab::of(Some(&u.item));
        // The tag selector takes no "match contents" (`FilterUpgradeWidgetBase.mouseClicked`).
        if has_tag_selector(tab) && s[1] == 2 && s[0] == 2 {
            s[0] = 0;
        }
        set_filter_settings(u, s);
        if has_tag_selector(tab) && k == 1 && (was == 2) != (s[1] == 2) {
            with(|st| st.reopen_after.insert(uuid));
        }
    });
    if with(|st| st.reopen_after.remove(&uuid)) {
        reopen_menu(game, uuid);
    }
}

/// The switch at the side of a closed upgrade's slot (`renderEnableButton`).
fn toggle_enabled(game: &mut Game<'_>, uuid: u128, i: usize) {
    edit_upgrade(game, uuid, i, |u| {
        if can_switch_off(&u.item) {
            let on = upgrade_enabled(u);
            set_upgrade_enabled(u, !on);
        }
    });
}

/// Changes upgrade slot `i` of `uuid`'s open menu, and tells the screen.
fn edit_upgrade(game: &mut Game<'_>, uuid: u128, i: usize, f: impl FnOnce(&mut ItemStack)) {
    if with(|s| s.settings.contains_key(&uuid)) {
        return;
    }
    let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
        return;
    };
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    let Some((mut items, carried)) =
        game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
    else {
        return;
    };
    if items.len() < tier.menu_slots() || i >= tier.upgrades {
        return;
    }
    let Some(upgrade) = items[tier.storage + i].as_mut() else {
        return;
    };
    f(upgrade);
    with(|s| s.extras.insert(uuid, (tier, items[tier.storage..].to_vec())));
    if let Some(host) = game.lan.as_mut() {
        horizon_client::lan::containers::travelersbackpack_menu::set_items(&mut host.hub, conn, items, carried);
    }
    resend_data(game, uuid);
}

/// A filter button's icons by state (`ButtonStates`).
fn filter_icons(tab: tb::Tab, k: usize) -> Vec<[f32; 2]> {
    match (tab, k) {
        (tb::Tab::Void | tb::Tab::Pickup, 0) => vec![[96.0, 0.0], [114.0, 0.0], [132.0, 0.0]],
        (_, 0) => vec![[96.0, 0.0], [114.0, 0.0]],
        (tb::Tab::Feeding, 1) => vec![[150.0, 0.0], [150.0, 18.0], [150.0, 36.0]],
        (tb::Tab::Feeding, _) => vec![[168.0, 18.0], [168.0, 0.0]],
        (_, 1) => vec![[96.0, 18.0], [114.0, 18.0], [186.0, 18.0]],
        _ => vec![[96.0, 36.0], [114.0, 36.0]],
    }
}

/// A filter button's tooltips by state (`WHITELIST_TOOLTIPS` and the others).
fn filter_tooltips(tab: tb::Tab, k: usize) -> Vec<&'static str> {
    match (tab, k) {
        (tb::Tab::Void, 0) => vec!["filter_allow_voiding", "filter_block_voiding", "filter_match_contents_voiding"],
        (tb::Tab::Pickup, 0) => vec!["filter_allow", "filter_block", "filter_match_contents"],
        (_, 0) => vec!["filter_allow", "filter_block"],
        (tb::Tab::Feeding, 1) => vec!["filter_always_eat", "filter_half_nutrition", "filter_full_nutrition"],
        (tb::Tab::Feeding, _) => vec!["filter_block_bad_effects", "filter_allow_bad_effects"],
        (_, 1) => vec!["filter_item", "filter_modid", "filter_tag"],
        _ => vec!["filter_match_components", "filter_ignore_components"],
    }
}

// ---- the hose (`HoseItem`): the worn backpack's tanks, sucked up, spilled, drunk -------------

const HOSE: &str = "travelersbackpack:hose";
/// Where a hose keeps its mode and tank (`HOSE_MODES`): its custom data.
const HOSE_KEY: &str = "TravelersBackpackHose";
const HOSE_MODE_CHANNEL: &str = "travelersbackpack:hose_mode";
const HOSE_TANK_CHANNEL: &str = "travelersbackpack:hose_tank";
const SUCK_MODE: i32 = 1;
const SPILL_MODE: i32 = 2;
const DRINK_MODE: i32 = 3;
/// `Player.blockInteractionRange`.
const HOSE_REACH: f64 = 4.5;

/// A hose's mode and tank (1 the left, 2 the right), suck and left as it comes.
fn hose_settings(stack: &ItemStack) -> (i32, i32) {
    match custom_data(stack).get(&horizon_world::nbt::JavaString::from(HOSE_KEY)) {
        Some(Tag::IntArray(v)) if v.len() == 2 => (v[0].clamp(1, 3), v[1].clamp(1, 2)),
        _ => (SUCK_MODE, 1),
    }
}

/// Sets them, and the model the mode shows (its custom model data's string).
fn set_hose_settings(stack: &mut ItemStack, mode: i32, tank: i32) {
    let mut data = custom_data(stack);
    data.insert(HOSE_KEY.into(), Tag::IntArray(vec![mode, tank]));
    set_custom_data(stack, data);
    let name = match mode {
        SPILL_MODE => "spill",
        DRINK_MODE => "drink",
        _ => "suck",
    };
    let mut look = Compound::new();
    look.insert("strings".into(), Tag::List(vec![Tag::String(name.into())]));
    let d = stack.data_mut();
    d.set_other("minecraft:custom_model_data", Some(Tag::Compound(look)));
    d.wire = Default::default();
    stack.tidy();
}

/// The mode and tank keys' lines (`item.travelersbackpack.hose.*`).
fn hose_mode_key(mode: i32) -> &'static str {
    match mode {
        SPILL_MODE => "item.travelersbackpack.hose.spill",
        DRINK_MODE => "item.travelersbackpack.hose.drink",
        _ => "item.travelersbackpack.hose.suck",
    }
}

fn hose_tank_key(tank: i32) -> &'static str {
    if tank == 1 { "item.travelersbackpack.hose.tank_left" } else { "item.travelersbackpack.hose.tank_right" }
}

/// `ServerActions.switchHoseMode`, `switchHoseTank`: the hose in the hand.
fn hose_switch(game: &mut Game<'_>, player: u128, tank: bool, value: i32) {
    let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == player)) else {
        return;
    };
    let slot = g.selected;
    let Some(hose) = g.inventory.get_mut(slot).and_then(Option::as_mut).filter(|s| &*s.item == HOSE) else {
        return;
    };
    let (mode, which) = hose_settings(hose);
    if tank {
        set_hose_settings(hose, mode, value.clamp(1, 2));
    } else {
        set_hose_settings(hose, value.clamp(SUCK_MODE, DRINK_MODE), which);
    }
}

/// The worn backpack's tanks, when it wears one with the tanks upgrade and
/// its menu isn't open: the backpack, its slots, its tier, the upgrade's slot.
fn worn_tanks(game: &Game<'_>, uuid: u128) -> Option<(ItemStack, Vec<Option<ItemStack>>, tb::Tier, usize)> {
    if worn_menu_open(uuid) {
        return None;
    }
    let stack = worn(game, uuid)?;
    let tier = stack_tier(&stack);
    let items = contents(&stack, tier.slots());
    let k = tier.storage + tanks_slot(&items, tier)?;
    Some((stack, items, tier, k))
}

fn save_worn_tanks(game: &mut Game<'_>, uuid: u128, mut stack: ItemStack, mut items: Vec<Option<ItemStack>>, k: usize, tanks: &Tanks) {
    if let Some(upgrade) = items[k].as_mut() {
        set_tanks(upgrade, tanks);
    }
    set_contents(&mut stack, items);
    set_worn(game, uuid, Some(&stack));
    tell_worn(game, uuid);
}

/// The blocks along player `uuid`'s view, nearest first, within reach.
fn hose_ray(game: &Game<'_>, uuid: u128) -> Vec<[i32; 3]> {
    // The acting player's turn: their pose is the game's.
    let _ = uuid;
    let Some(eye) = game.eye_position() else {
        return Vec::new();
    };
    let (yaw, pitch) = (f64::from(game.yaw).to_radians(), f64::from(game.pitch).to_radians());
    let dir = [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()];
    let mut out: Vec<[i32; 3]> = Vec::new();
    let mut t = 0.0;
    while t <= HOSE_REACH {
        let p = [0, 1, 2].map(|i| (eye[i] + dir[i] * t).floor() as i32);
        if out.last() != Some(&p) {
            out.push(p);
        }
        t += 0.05;
    }
    out
}

fn is_fluid_block(block: &str) -> bool {
    block == "minecraft:water" || block == "minecraft:lava"
}

/// A block state with one property changed, if it has it.
fn with_property(game: &Game<'_>, pos: [i32; 3], key: &str, value: &str) -> Option<horizon_world::registry::BlockStateId> {
    let registry = game.pack.assets.registry;
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let mut wanted = registry.state(state)?.clone();
    wanted.properties.get(key)?;
    wanted.properties.insert(key.into(), value.into());
    registry.resolve(&wanted).ok()
}

/// `HoseItem.use` with the hose in the main hand of a player who wears a
/// backpack with tanks: sucking a source block (or a waterlogged block's
/// water) into the chosen tank, or spilling a bucket of it where the view
/// meets a block. Drinking is a use over time (`hose_use_*`).
fn hose_use(game: &mut Game<'_>, claimed: &mut bool) {
    let uuid = game.native_acting_id();
    let Some(held) = game.inventory.get(game.selected).cloned().flatten().filter(|s| &*s.item == HOSE) else {
        return;
    };
    let (mode, which) = hose_settings(&held);
    if mode == DRINK_MODE {
        return;
    }
    *claimed = true;
    // One click is heard twice (on a block, then in the air).
    let tick = game.world.game_time;
    if with(|s| s.hose_tick.insert(uuid, tick) == Some(tick)) {
        return;
    }
    let Some((stack, items, tier, k)) = worn_tanks(game, uuid) else {
        return;
    };
    let Some(upgrade) = items[k].as_ref() else {
        return;
    };
    let mut tanks = tanks_of(upgrade);
    let side = usize::try_from(which - 1).unwrap_or(0).min(1);
    let capacity = tank_capacity(tier);
    let dimension = game.dim.id().to_owned();
    let ray = hose_ray(game, uuid);
    let mut changed = false;
    if mode == SUCK_MODE {
        for &pos in &ray {
            let block = game.native_block(pos);
            let level = game.native_block_property(pos, "level");
            // `ClipContext.Fluid.SOURCE_ONLY`: a flowing fluid is looked through.
            let (fluid, emptied) = if is_fluid_block(&block) && level.as_deref() == Some("0") {
                (block.to_string(), game.native_default_state("minecraft:air"))
            } else if is_fluid_block(&block) || block.ends_with("air") {
                continue;
            } else if game.native_block_property(pos, "waterlogged").as_deref() == Some("true") {
                ("minecraft:water".to_owned(), with_property(game, pos, "waterlogged", "false"))
            } else {
                break;
            };
            let tank = &mut tanks[side];
            let fits = (tank.0.is_empty() || tank.0 == fluid) && tank.1 + BUCKET <= capacity;
            if fits && let Some(state) = emptied && game.native_set_block(&dimension, pos, state, 3) {
                tank.0 = fluid.clone();
                tank.1 += BUCKET;
                changed = true;
                let sound = if fluid == "minecraft:lava" { "minecraft:item.bucket.fill_lava" } else { "minecraft:item.bucket.fill" };
                let at = pos.map(|c| f64::from(c) + 0.5);
                play_near(game, uuid, sound, "block", Some(at), 1.0, 1.0);
            }
            break;
        }
    } else if mode == SPILL_MODE {
        let fluid = tanks[side].0.clone();
        if let Some((bottle @ ("minecraft:splash_potion" | "minecraft:lingering_potion"), id)) = potion_fluid(&fluid)
            && tanks[side].1 >= BOTTLE
        {
            let thrown = ItemStack::new(bottle, 1).with_potion(id);
            throw_potion(game, uuid, thrown);
            if !game.creative {
                tanks[side].1 -= BOTTLE;
                if tanks[side].1 == 0 {
                    tanks[side].0.clear();
                }
            }
            save_worn_tanks(game, uuid, stack, items, k, &tanks);
            advance(game, uuid, "hose_spill_potion");
            return;
        }
        if is_fluid_block(&fluid) && tanks[side].1 >= BUCKET {
            // `ClipContext.Fluid.NONE`: the first block met, and the place before it.
            let mut before = None;
            for &pos in &ray {
                let block = game.native_block(pos);
                if block.ends_with("air") || is_fluid_block(&block) {
                    before = Some(pos);
                    continue;
                }
                // Water into a waterloggable block (`LiquidBlockContainer`).
                let into = (fluid == "minecraft:water")
                    .then(|| with_property(game, pos, "waterlogged", "true"))
                    .flatten();
                let placed = match (into, before) {
                    (Some(state), _) => game.native_set_block(&dimension, pos, state, 3).then_some(pos),
                    // Water boils away in the Nether (`WATER_EVAPORATES`).
                    (None, Some(at)) if fluid == "minecraft:water" && dimension == "minecraft:the_nether" => {
                        let at3 = at.map(|c| f64::from(c) + 0.5);
                        play_near(game, uuid, "minecraft:block.fire.extinguish", "block", Some(at3), 0.5, 2.6);
                        Some(at)
                    }
                    (None, Some(at)) => game
                        .native_default_state(&fluid)
                        .filter(|&s| game.native_set_block(&dimension, at, s, 3))
                        .map(|_| at),
                    (None, None) => None,
                };
                if let Some(at) = placed {
                    tanks[side].1 -= BUCKET;
                    if tanks[side].1 == 0 {
                        tanks[side].0.clear();
                    }
                    changed = true;
                    let sound = if fluid == "minecraft:lava" { "minecraft:item.bucket.empty_lava" } else { "minecraft:item.bucket.empty" };
                    play_near(game, uuid, sound, "block", Some(at.map(|c| f64::from(c) + 0.5)), 1.0, 1.0);
                }
                break;
            }
        }
    }
    if changed {
        save_worn_tanks(game, uuid, stack, items, k, &tanks);
        advance(game, uuid, if mode == SUCK_MODE { "hose_suck" } else { "hose_spill" });
    }
}

/// `ActionTypeTrigger`: the mod's criteria trigger `travelersbackpack:action_type`
/// with action `action` for player `uuid` (the advancements of its tree).
fn advance(game: &mut Game<'_>, uuid: u128, action: &str) {
    game.native_trigger_criterion(uuid, "travelersbackpack:action_type", |c| c.conditions["action"].as_str() == Some(action));
}

/// What the chosen tank gives to drink (`EffectFluidRegistry`): water, lava
/// and milk a bucket at a time, a potion a bottle.
fn drinkable(tank: &(String, i32)) -> Option<i32> {
    let need = if potion_fluid(&tank.0).is_some() { BOTTLE } else { BUCKET };
    let known = matches!(&*tank.0, "minecraft:water" | "minecraft:lava" | "minecraft:milk") || potion_fluid(&tank.0).is_some();
    (known && tank.1 >= need).then_some(need)
}

fn hose_drinking(stack: &ItemStack) -> Option<i32> {
    (&*stack.item == HOSE && hose_settings(stack).0 == DRINK_MODE).then_some(hose_settings(stack).1)
}

/// A hose drinks as a potion is drunk (`consumeTicks`: 1.6 seconds).
pub(super) fn use_duration(stack: &ItemStack, duration: &mut i32) {
    if hose_drinking(stack).is_some() {
        *duration = 32;
    }
}

pub(super) fn use_animation(stack: &ItemStack, animation: &mut horizon_world::consumable::UseAnimation) {
    if hose_drinking(stack).is_some() {
        *animation = horizon_world::consumable::UseAnimation::Drink;
    }
}

/// `HoseItem.drink`: no drinking from a tank with nothing to drink.
pub(super) fn use_item_start(game: &mut Game<'_>, stack: &ItemStack, canceled: &mut bool) {
    let Some(which) = hose_drinking(stack) else {
        return;
    };
    let uuid = game.native_acting_id();
    let ok = worn_tanks(game, uuid)
        .and_then(|(_, items, _, k)| items[k].as_ref().map(tanks_of))
        .and_then(|t| drinkable(&t[usize::try_from(which - 1).unwrap_or(0).min(1)]))
        .is_some();
    if !ok {
        *canceled = true;
    }
}

/// `HoseItem.finishUsingItem`: the tank's fluid acts on the drinker
/// (`WaterEffect`, `LavaEffect`, `MilkEffect`, `PotionEffect`) and so much of
/// it is gone.
pub(super) fn use_item_finish(game: &mut Game<'_>, stack: &ItemStack) {
    let Some(which) = hose_drinking(stack) else {
        return;
    };
    let uuid = game.native_acting_id();
    let Some((worn_stack, items, _, k)) = worn_tanks(game, uuid) else {
        return;
    };
    let Some(mut tanks) = items[k].as_ref().map(tanks_of) else {
        return;
    };
    let side = usize::try_from(which - 1).unwrap_or(0).min(1);
    let Some(need) = drinkable(&tanks[side]) else {
        return;
    };
    let fluid = tanks[side].0.clone();
    let effect = |id: &str, amplifier: u8, duration: i32| super::native_player_state::NativeEffect {
        id: id.to_owned(),
        amplifier,
        duration,
        ambient: false,
        particles: true,
    };
    match &*fluid {
        "minecraft:water" => {
            let burning = game.player.as_ref().is_some_and(|p| p.fire_ticks > 0);
            if burning {
                if let Some(p) = game.player.as_mut() {
                    let ticks = p.fire_ticks.min(0);
                    p.set_remaining_fire_ticks(ticks);
                }
            } else {
                // A biome of 2.0 or more (`getBaseTemperature`).
                const HOT: [&str; 11] = [
                    "minecraft:desert",
                    "minecraft:savanna",
                    "minecraft:savanna_plateau",
                    "minecraft:windswept_savanna",
                    "minecraft:badlands",
                    "minecraft:eroded_badlands",
                    "minecraft:wooded_badlands",
                    "minecraft:nether_wastes",
                    "minecraft:soul_sand_valley",
                    "minecraft:crimson_forest",
                    "minecraft:warped_forest",
                ];
                let at = game.player.as_ref().map(|p| [p.position.x, p.position.y, p.position.z]);
                let biome = at.and_then(|p| game.native_biome(p.map(|c| c.floor() as i32)));
                if biome.is_some_and(|b| HOT.contains(&&*b) || b == "minecraft:basalt_deltas") {
                    game.native_add_effect(uuid, &effect("minecraft:regeneration", 0, 140));
                }
            }
        }
        "minecraft:lava" => {
            if let Some(p) = game.player.as_mut() {
                p.set_remaining_fire_ticks(300);
            }
            game.native_add_effect(uuid, &effect("minecraft:speed", 2, 1200));
            game.native_add_effect(uuid, &effect("minecraft:jump_boost", 0, 1200));
            game.native_add_effect(uuid, &effect("minecraft:mining_fatigue", 3, 1200));
        }
        "minecraft:milk" => {
            game.native_set_effects(uuid, &[]);
        }
        potion => {
            let Some((_, id)) = potion_fluid(potion) else {
                return;
            };
            // As a bottle of it is drunk (`PotionContents.onConsume`).
            let bottle = ItemStack::new("minecraft:potion", 1).with_potion(id);
            let (lasting, now) = horizon_client::effects::PlayerEffects::contents_effects(
                &horizon_client::brewing::contents_effects(&bottle),
                bottle.potion_duration_scale(),
            );
            for e in lasting {
                game.add_player_effect(e);
            }
            for a in now {
                game.apply_effect_action(a);
            }
        }
    }
    tanks[side].1 -= need;
    if tanks[side].1 <= 0 {
        tanks[side] = (String::new(), 0);
    }
    save_worn_tanks(game, uuid, worn_stack, items, k, &tanks);
    if potion_fluid(&fluid).is_some() {
        advance(game, uuid, "hose_drink_potion");
    }
    advance(game, uuid, "hose_drink");
}

// ---- tooltips (`appendHoverText`s, `ClientBackpackTooltipComponent`) ----------------------

/// Colours as `HudBatch` packs them (`rgba`: alpha, blue, green, red).
const BLUE: u32 = 0xFFFF_5555;
const WHITE: u32 = 0xFFFF_FFFF;
const DARK_PURPLE: u32 = 0xFFAA_00AA;
/// `magnetUpgradeSettings.pullRange`'s default.
const MAGNET_PULL_RANGE: i32 = 5;

/// A translated line with its `§` codes.
fn legacy(game: &Game<'_>, key: &str) -> Text {
    Text::legacy(&tr(game, key))
}

/// `String.format` of a translation: `%s`, `%1$s`, `%%`.
fn format_args(template: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut next = 0;
    let mut rest = template;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        rest = &rest[i + 1..];
        if let Some(r) = rest.strip_prefix('%') {
            out.push('%');
            rest = r;
            continue;
        }
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        let index = if digits > 0 && rest[digits..].starts_with("$s") {
            let n: usize = rest[..digits].parse().unwrap_or(1);
            rest = &rest[digits + 2..];
            n.saturating_sub(1)
        } else if let Some(r) = rest.strip_prefix('s') {
            rest = r;
            next += 1;
            next - 1
        } else {
            out.push('%');
            continue;
        };
        out.push_str(args.get(index).map_or("", String::as_str));
    }
    out.push_str(rest);
    out
}

/// A backpack's special ability (`BackpackAbilities`): whether it has a
/// description of its own, its effects, its attribute modifiers
/// (attribute, amount, operation), and whether it works worn and placed.
struct Ability {
    custom: bool,
    effects: &'static [&'static str],
    attributes: &'static [(&'static str, f64, u8)],
    item: bool,
    block: bool,
}

fn ability(design: &str) -> Option<Ability> {
    let a = |custom, effects, attributes, item, block| Some(Ability { custom, effects, attributes, item, block });
    match design {
        "netherite" => a(false, &[], &[("armor", 4.0, 0)], true, false),
        "diamond" => a(false, &[], &[("armor", 3.0, 0)], true, false),
        "gold" | "iron" => a(false, &[], &[("armor", 2.0, 0)], true, false),
        "emerald" => a(false, &[], &[("luck", 1.0, 0)], true, false),
        "quartz" => a(false, &["haste"], &[], true, false),
        "redstone" | "bookshelf" | "sponge" | "melon" => a(true, &[], &[], false, true),
        "cactus" => a(true, &[], &[], true, true),
        "lapis" | "cake" | "hay" | "pumpkin" | "creeper" | "blaze" | "ghast" | "spider" | "wither" | "bee" | "ocelot"
        | "cow" | "chicken" => a(true, &[], &[], true, false),
        "enderman" => a(true, &[], &[("block_interaction_range", 1.0, 0), ("entity_interaction_range", 1.0, 0)], true, false),
        "warden" => a(true, &[], &[("max_health", 4.0, 0)], true, false),
        "dragon" => a(false, &["regeneration", "strength"], &[], true, false),
        "magma_cube" => a(false, &["fire_resistance"], &[], true, false),
        "bat" => a(false, &["night_vision"], &[], true, false),
        "squid" => a(false, &["water_breathing", "night_vision"], &[], true, false),
        "fox" => a(false, &["jump_boost"], &[("movement_speed", 0.1, 2)], true, false),
        _ => None,
    }
}

/// `ItemAttributeModifiers.ATTRIBUTE_MODIFIER_FORMAT` (`#.##`).
fn modifier_number(v: f64) -> String {
    let s = format!("{:.2}", (v * 100.0).round() / 100.0);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A fluid's name, as `CommonFluid.getFluidName`.
fn fluid_name(game: &Game<'_>, fluid: &str) -> String {
    match fluid_code(fluid) {
        1 => tr(game, "block.minecraft.water"),
        2 => tr(game, "block.minecraft.lava"),
        3 => tr(game, "fluid_type.travelersbackpack.milk"),
        _ => tr(game, "fluid_type.travelersbackpack.potion"),
    }
}

/// A backpack item's storage (like stacks merged), upgrades, tools and tanks.
struct Inside {
    storage: Vec<ItemStack>,
    upgrades: Vec<ItemStack>,
    tools: Vec<ItemStack>,
    fluids: Vec<(String, i32)>,
}


fn inside_of(stack: &ItemStack) -> Inside {
    let tier = stack_tier(stack);
    let items = contents(stack, tier.slots());
    let part = |r: std::ops::Range<usize>| items[r].iter().flatten().cloned().collect::<Vec<_>>();
    let upgrades = part(tier.storage..tier.storage + tier.upgrades);
    let fluids = upgrades
        .iter()
        .find(|u| &*u.item == TANKS_UPGRADE)
        .map(|u| tanks_of(u).into_iter().filter(|t| t.1 > 0).collect())
        .unwrap_or_default();
    Inside {
        storage: merge_stacks(part(0..tier.storage)),
        upgrades,
        tools: part(tier.storage + tier.upgrades..tier.slots()),
        fluids,
    }
}

/// `BackpackTooltipComponent.mergeStacks`: like stacks as one, split at 999.
fn merge_stacks(stacks: Vec<ItemStack>) -> Vec<ItemStack> {
    let mut unique: Vec<ItemStack> = Vec::new();
    for s in stacks {
        match unique.iter_mut().find(|u| same_item(u, &s)) {
            Some(u) => u.count += s.count,
            None => unique.push(s),
        }
    }
    let mut out = Vec::new();
    for s in unique {
        let mut left = s.count;
        while left > 999 {
            out.push(ItemStack { count: 999, ..s.clone() });
            left -= 999;
        }
        if left > 0 {
            out.push(ItemStack { count: left, ..s });
        }
    }
    out
}

/// The lines each of the mod's items adds to its tooltip.
pub(super) fn tooltip(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    let mut add: Vec<(Text, u32)> = Vec::new();
    let item = &*stack.item;
    if item == HOSE {
        // `HoseItem`: its mode and tank.
        let (mode, tank) = hose_settings(stack);
        add.push((Text::literal(tr(game, hose_mode_key(mode))), BLUE));
        add.push((Text::literal(tr(game, hose_tank_key(tank))), BLUE));
    } else if tb::is_backpack(item) {
        // The storage is the mod's (`BACKPACK_CONTAINER`), not a shulker
        // box's: no "count x name" lines for it.
        let name = |s: &ItemStack| s.custom_name().or_else(|| s.item_name()).unwrap_or_else(|| game.pack.names.display_item(&s.item));
        let translate = |k: &str| Some(game.translate(k, if k.ends_with("count") { "%s x %s" } else { "and %s more..." }));
        let listed = horizon_client::item_tooltip::container_lines(stack, &name, &translate);
        if !listed.is_empty()
            && let Some(at) = (0..lines.len().saturating_sub(listed.len() - 1))
                .find(|&i| lines[i..i + listed.len()].iter().zip(&listed).all(|((t, _), l)| t.plain() == *l))
        {
            lines.drain(at..at + listed.len());
        }
        backpack_tooltip(game, stack, &mut add);
    } else if let Some(name) = item.strip_prefix("travelersbackpack:") {
        upgrade_tooltip(game, stack, name, &mut add);
    }
    if add.is_empty() {
        return;
    }
    // Before the advanced lines (the item's id), where `appendHoverText` puts them.
    let id = stack.item.to_string();
    let at = lines.iter().position(|(t, _)| t.plain() == id).unwrap_or(lines.len()).max(lines.len().min(1));
    let add: Vec<(Text, u32)> = add.into_iter().flat_map(|(t, c)| wrap(game, &t, TOOLTIP_WIDTH).into_iter().map(move |t| (t, c))).collect();
    lines.splice(at..at, add);
}

/// The widest a tooltip line of the mod gets before it wraps, in GUI pixels
/// (long translations ran off the screen).
const TOOLTIP_WIDTH: f32 = 300.0;

/// `text` broken at spaces into lines at most `max` wide, each span keeping its style.
fn wrap(game: &Game<'_>, text: &Text, max: f32) -> Vec<Text> {
    if game.native_text_width(&text.plain()) <= max {
        return vec![text.clone()];
    }
    let mut lines = vec![Text::default()];
    let mut width = 0.0;
    for span in &text.spans {
        for (i, word) in span.text.split(' ').enumerate() {
            let piece = if i == 0 { word.to_owned() } else { format!(" {word}") };
            let w = game.native_text_width(&piece);
            let piece = if width + w > max && width > 0.0 {
                lines.push(Text::default());
                width = 0.0;
                word.to_owned()
            } else {
                piece
            };
            width += game.native_text_width(&piece);
            let line = lines.last_mut().expect("a line");
            match line.spans.last_mut() {
                Some(last) if last.style == span.style => last.text.push_str(&piece),
                _ => line.spans.push(horizon_client::text::Span { text: piece, style: span.style.clone() }),
            }
        }
    }
    lines
}

/// `TravelersBackpackItem.appendHoverText`.
fn backpack_tooltip(game: &Game<'_>, stack: &ItemStack, add: &mut Vec<(Text, u32)>) {
    let design = stack.item.strip_prefix("travelersbackpack:").unwrap_or_default();
    let tier = stack_tier(stack);
    let tier_line = format!("{}{}", tr(game, "tier.travelersbackpack.backpack"), tr(game, &format!("tier.travelersbackpack.{}", tier.name)));
    add.push((Text::legacy(&tier_line), WHITE));
    let filled = stack.data().is_some_and(|d| !d.container.is_empty());
    if filled && !game.native_has_control_down() {
        add.push((legacy(game, "item.travelersbackpack.inventory_tooltip"), BLUE));
    }
    if cfg().obtain_tips && matches!(design, "bat" | "villager" | "iron_golem") {
        add.push((legacy(game, &format!("obtain.travelersbackpack.{design}")), BLUE));
    }
    let Some(ability) = ability(design) else {
        return;
    };
    if !game.native_has_shift_down() {
        add.push((legacy(game, "ability.travelersbackpack.hold_shift"), BLUE));
        return;
    }
    if ability.custom {
        add.push((legacy(game, &format!("ability.travelersbackpack.{design}")), BLUE));
    }
    if !ability.effects.is_empty() || !ability.attributes.is_empty() {
        add.push((legacy(game, "ability.travelersbackpack.when_equipped"), DARK_PURPLE));
    }
    for effect in ability.effects {
        add.push((Text::literal(format!("- {}", tr(game, &format!("effect.minecraft.{effect}")))), BLUE));
    }
    for &(attribute, amount, op) in ability.attributes {
        let shown = if op == 0 { amount } else { amount * 100.0 };
        let template = tr(game, &format!("attribute.modifier.plus.{op}"));
        let name = tr(game, &format!("attribute.name.{attribute}"));
        add.push((Text::legacy(&format_args(&template, &[modifier_number(shown), name])), BLUE));
    }
    let key = match (ability.item, ability.block) {
        (true, true) => "ability.travelersbackpack.item_and_block",
        (false, true) => "ability.travelersbackpack.block",
        _ => "ability.travelersbackpack.item",
    };
    add.push((legacy(game, key), WHITE));
}

/// `UpgradeItem`, `TierUpgrade`, `MagnetUpgradeItem`, `LanternUpgradeItem`,
/// `TanksUpgradeItem`, `SleepingBagItem`.
fn upgrade_tooltip(game: &Game<'_>, stack: &ItemStack, name: &str, add: &mut Vec<(Text, u32)>) {
    let tier_upgrade = |from: &str| {
        let t = tr(game, &format!("tier.travelersbackpack.{from}"));
        // The tier's own colour stays on it (a component of its own there): blue after.
        let t = format!("{t}§9");
        Text::legacy(&format_args(&tr(game, "item.travelersbackpack.tier_upgrade_tooltip"), &[t]))
    };
    let apply = |add: &mut Vec<(Text, u32)>| add.push((legacy(game, "item.travelersbackpack.upgrade_apply_tooltip"), WHITE));
    match name {
        "blank_upgrade" => add.push((legacy(game, "item.travelersbackpack.blank_upgrade_tooltip"), BLUE)),
        "iron_tier_upgrade" => add.push((tier_upgrade("leather"), BLUE)),
        "gold_tier_upgrade" => add.push((tier_upgrade("iron"), BLUE)),
        "diamond_tier_upgrade" => add.push((tier_upgrade("gold"), BLUE)),
        "netherite_tier_upgrade" => add.push((tier_upgrade("diamond"), BLUE)),
        "magnet_upgrade" => {
            let text = format_args(&tr(game, "item.travelersbackpack.magnet_upgrade_tooltip"), &[MAGNET_PULL_RANGE.to_string()]);
            add.push((Text::legacy(&text), BLUE));
            apply(add);
        }
        "lantern_upgrade" => {
            // Active while the dynamic lights are on (Horizon's or Better Dynamic Lights').
            let line = if light_rules(game).is_some() { "active" } else { "missing" };
            add.push((legacy(game, &format!("item.travelersbackpack.lantern_upgrade_{line}")), WHITE));
            add.push((legacy(game, "item.travelersbackpack.lantern_upgrade_tooltip"), BLUE));
            apply(add);
        }
        "tanks_upgrade" | "crafting_upgrade" | "furnace_upgrade" | "smoker_upgrade" | "blast_furnace_upgrade" | "pickup_upgrade"
        | "jukebox_upgrade" | "void_upgrade" | "feeding_upgrade" | "refill_upgrade" => {
            add.push((legacy(game, &format!("item.travelersbackpack.{name}_tooltip")), BLUE));
            apply(add);
            if name == "tanks_upgrade" {
                for (fluid, amount) in tanks_of(stack).into_iter().filter(|t| t.1 > 0) {
                    add.push((Text::literal(format!("{}: {}mB", fluid_name(game, &fluid), mb(amount))), BLUE));
                }
            }
        }
        n if n.ends_with("_sleeping_bag") => add.push((legacy(game, "item.travelersbackpack.sleeping_bag_attach"), WHITE)),
        _ => {}
    }
}

/// The picture under a backpack's tooltip while Ctrl is held
/// (`ClientBackpackTooltipComponent.getWidth`, `getHeight`).
pub(super) fn tooltip_component(game: &Game<'_>, stack: &ItemStack, size: &mut Option<[f32; 2]>) {
    if !tb::is_backpack(&stack.item) || !game.native_has_control_down() {
        return;
    }
    let inside = inside_of(stack);
    let mut h = 10.0 * inside.fluids.len() as f32;
    if !inside.upgrades.is_empty() {
        h += 28.0;
    }
    if !inside.storage.is_empty() {
        h += 10.0 + 18.0 * inside.storage.len().div_ceil(9) as f32;
    }
    if !inside.tools.is_empty() {
        h += 28.0;
    }
    let mut w = 20.0 * inside.storage.len().min(9) as f32;
    for (fluid, amount) in &inside.fluids {
        w = w.max(game.native_text_width(&format!("{}: {}mB", fluid_name(game, fluid), mb(*amount))));
    }
    w = w.max(18.0 * inside.upgrades.len().max(inside.tools.len()) as f32);
    if h > 0.0 && w > 0.0 {
        *size = Some([w, h]);
    }
}

/// `ClientBackpackTooltipComponent.extractText`, `extractImage`.
pub(super) fn tooltip_component_draw(game: &Game<'_>, stack: &ItemStack, batch: &mut horizon_client::hud::HudBatch, origin: [f32; 2]) {
    if !tb::is_backpack(&stack.item) || !game.native_has_control_down() {
        return;
    }
    let inside = inside_of(stack);
    let [x, mut y] = origin;
    for (fluid, amount) in &inside.fluids {
        let name = format!("{}: ", fluid_name(game, fluid));
        game.native_text(batch, &name, [x, y], 0x00FF_FFFF, true);
        game.native_text(batch, &format!("{}mB", mb(*amount)), [x + game.native_text_width(&name), y], 0x0055_55FF, true);
        y += 10.0;
    }
    let row = |batch: &mut horizon_client::hud::HudBatch, y: &mut f32, title: &str, items: &[ItemStack], step: f32| {
        if items.is_empty() {
            return;
        }
        game.native_text(batch, &tr(game, title), [x, *y], 0x00FF_FF55, true);
        *y += 10.0;
        for (i, s) in items.iter().enumerate() {
            let (col, line) = (i % 9, i / 9);
            game.native_gui_draw_item(batch, s, [x + step * col as f32, *y + 18.0 * line as f32]);
        }
        *y += 18.0 * items.len().div_ceil(9) as f32;
    };
    row(batch, &mut y, "screen.travelersbackpack.upgrades", &inside.upgrades, 18.0);
    row(batch, &mut y, "screen.travelersbackpack.inventory", &inside.storage, 20.0);
    row(batch, &mut y, "screen.travelersbackpack.tools", &inside.tools, 18.0);
}


// ---- dyeing (`DataComponents.DYED_COLOR` on the standard backpack) ------------------------------

const STANDARD: &str = "travelersbackpack:standard";
/// Where a placed backpack keeps its dye (its block entity's compound).
const DYE_NBT: &str = "DyedColor";

/// The standard backpack's dye, 0xRRGGBB.
fn item_dye(stack: &ItemStack) -> Option<i32> {
    stack.data().and_then(|d| d.dyed_color).filter(|_| &*stack.item == STANDARD)
}

fn set_item_dye(stack: &mut ItemStack, dye: Option<i32>) {
    if &*stack.item != STANDARD {
        return;
    }
    let d = stack.data_mut();
    d.dyed_color = dye;
    d.wire = Default::default();
    stack.tidy();
}

fn block_dye(nbt: &Compound) -> Option<i32> {
    match nbt.get(&horizon_world::nbt::JavaString::from(DYE_NBT)) {
        Some(Tag::Int(c)) => Some(*c),
        _ => None,
    }
}

fn set_block_dye(nbt: &mut Compound, dye: Option<i32>) {
    match dye {
        Some(c) => {
            nbt.insert(DYE_NBT.into(), Tag::Int(c));
        }
        None => {
            nbt.remove(&horizon_world::nbt::JavaString::from(DYE_NBT));
        }
    }
}

/// `backpack_base_dyed`: the body's boxes (from, to) and each face's texels
/// (down, up, north, south, west, east), on `block/backpack/dyed`.
const DYED_BODY: [([f32; 3], [f32; 3], [[f32; 4]; 6]); 6] = [
    ([4.1, 0.8, 5.0], [11.9, 7.8, 8.9], [[3.75, 2.255, 6.25, 3.505], [0.0, 0.0, 0.25, 0.25], [1.25, 3.5, 3.75, 5.75], [5.0, 3.5, 7.5, 5.75], [3.75, 3.5, 5.0, 5.75], [0.0, 3.5, 1.25, 5.75]]),
    ([4.1, 7.8, 5.0], [11.9, 10.1, 8.9], [[0.0, 0.0, 0.25, 0.25], [3.75, 1.25, 1.25, 0.0], [1.25, 1.25, 3.75, 2.0], [5.0, 1.25, 7.5, 2.0], [3.75, 1.25, 5.0, 2.0], [0.0, 1.25, 1.25, 2.0]]),
    ([4.1, 0.0, 5.0], [11.9, 0.8, 8.1], [[3.5, 8.5, 6.0, 9.5], [0.0, 0.0, 0.25, 0.25], [1.0, 9.5, 3.5, 9.75], [4.5, 9.5, 7.0, 9.75], [3.5, 9.5, 4.5, 9.75], [0.0, 9.5, 1.0, 9.75]]),
    ([4.9, 2.4, 8.9], [11.1, 7.1, 10.5], [[4.5, 6.0, 2.5, 6.5], [0.5, 6.0, 2.5, 6.5], [0.0, 0.0, 0.25, 0.25], [3.0, 6.5, 5.0, 8.0], [2.5, 6.5, 3.0, 8.0], [0.0, 6.5, 0.5, 8.0]]),
    ([4.8, 1.6, 4.2], [5.6, 7.8, 5.0], [[5.75, 6.0, 6.0, 6.25], [5.5, 6.0, 5.75, 6.25], [5.5, 6.25, 5.75, 8.25], [0.0, 0.0, 0.25, 0.25], [5.75, 6.25, 6.0, 8.25], [5.25, 6.25, 5.5, 8.25]]),
    ([10.4, 1.6, 4.2], [11.2, 7.8, 5.0], [[7.0, 6.0, 7.25, 6.25], [6.75, 6.0, 7.0, 6.25], [6.75, 6.25, 7.0, 8.25], [0.0, 0.0, 0.25, 0.25], [7.0, 6.25, 7.25, 8.25], [6.5, 6.25, 6.75, 8.25]]),
];

/// The dyed body in its colour (`BackpackDynamicModel`'s `BASE_DYED`, tint 0),
/// the model turned `turn` degrees as its block state turns it.
fn draw_dyed_body(game: &Game<'_>, draw: &mut super::native_block_entity_draw::BlockEntityDraw, colour: i32, turn: f32) {
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], -turn, [0.5, 0.5, 0.5]);
    let sprite = "travelersbackpack:block/backpack/dyed";
    for (from, to, uvs) in DYED_BODY {
        let faces = uvs.map(|uv| Some(super::native_block_entity_draw::CubeFace { sprite, uv: Some(uv) }));
        game.native_draw_cube(draw, from, to, faces, colour as u32 & 0x00FF_FFFF);
    }
    draw.pop();
}

/// `RightClickHandler`: a dyed standard backpack used on a cauldron with water
/// is washed (the game's `cauldron_can_remove_dye`); the advancement for it.
fn washed(game: &mut Game<'_>, pos: [i32; 3]) {
    let held = game.inventory.get(game.selected).cloned().flatten();
    if held.is_some_and(|s| item_dye(&s).is_some()) && game.native_block(pos) == "minecraft:water_cauldron" {
        let uuid = game.native_acting_id();
        advance(game, uuid, "undye_backpack");
    }
}

// ---- shears (`RightClickHandler`: back to the standard backpack) -------------------------------

/// The shears' durability (`Items.SHEARS`).
const SHEARS_DURABILITY: u32 = 238;

/// `RightClickHandler`: shears, sneaking, on a placed backpack of a design:
/// it drops as the standard backpack with its contents, tier, settings and
/// bag, its bag folded; the shears wear (`hurtAndBreak`).
fn shear(game: &mut Game<'_>, uuid: u128, dimension: &str, pos: [i32; 3]) {
    let Some((selected, creative)) =
        game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| (g.selected, g.game_mode == 1))
    else {
        return;
    };
    let shears = game
        .lan
        .as_ref()
        .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid))
        .and_then(|g| g.inventory.get(selected).cloned().flatten())
        .is_some_and(|s| &*s.item == "minecraft:shears");
    if !shears || game.native_block(pos) == STANDARD {
        return;
    }
    let Some(mut stack) = take_placed(game, dimension, pos) else {
        return;
    };
    stack.item = STANDARD.into();
    refresh_look(&mut stack);
    let at = pos.map(|c| f64::from(c) + 0.5);
    game.spawn_item(stack, at, super::dispenser::new_item_motion(), 10);
    play_near(game, uuid, "minecraft:entity.sheep.shear", "player", Some(at), 1.0, 1.0);
    if !creative {
        let broke = game
            .lan
            .as_mut()
            .and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid))
            .and_then(|g| {
                let slot = g.inventory.get_mut(selected)?;
                let s = slot.as_mut()?;
                s.damage += 1;
                (s.damage >= SHEARS_DURABILITY).then(|| *slot = None)
            })
            .is_some();
        if broke {
            play_near(game, uuid, "minecraft:entity.item.break", "player", Some(at), 0.8, 1.0);
        }
    }
    advance(game, uuid, "revert_custom_backpack");
}

/// `ServerActions.throwPotion`: the potion thrown from the eyes, as a splash
/// or lingering potion is (`spawnProjectileFromRotation`, -20 degrees up,
/// power 0.5), with its throw sound.
fn throw_potion(game: &mut Game<'_>, uuid: u128, potion: ItemStack) {
    use horizon_core::random::RandomSource as _;
    let Some(eye) = game.eye_position() else {
        return;
    };
    let sound = if &*potion.item == "minecraft:splash_potion" {
        "minecraft:entity.splash_potion.throw"
    } else {
        "minecraft:entity.lingering_potion.throw"
    };
    let pitch = 0.4 / (game.world.random.next_float() * 0.4 + 0.8);
    play_near(game, uuid, sound, "neutral", Some(eye), 0.5, pitch);
    let (yaw, look) = (game.yaw.to_radians(), game.pitch.to_radians());
    let dir = [-yaw.sin() * look.cos(), -(game.pitch - 20.0).to_radians().sin(), yaw.cos() * look.cos()].map(f64::from);
    let l = dir.iter().map(|c| c * c).sum::<f64>().sqrt().max(1e-9);
    let power = 0.5;
    let v = horizon_core::geometry::Vec3::new(dir[0] / l * power, dir[1] / l * power, dir[2] / l * power);
    let owner = Some(game.player_display_text());
    game.launch_thrown(potion, horizon_core::geometry::Vec3::new(eye[0], eye[1] - 0.1, eye[2]), v, owner);
}

// ---- special abilities (`BackpackAbilities`) ------------------------------------------------------

/// `COOLDOWNS`, in ticks: the original's `CooldownHelper.createCooldown` draws
/// from a fresh `RandomSource.create(1337)` each time, so each is this one.
fn cooldown_ticks(design: &str) -> Option<i32> {
    use horizon_core::random::RandomSource as _;
    if !["creeper", "cow", "chicken", "cake", "melon"].contains(&design) {
        return None;
    }
    let (_, min, max) = cfg().cooldowns.iter().find(|(d, _, _)| d.strip_prefix("travelersbackpack:") == Some(design))?;
    let (min, max) = (min.saturating_mul(20), max.saturating_mul(20));
    horizon_core::random::LegacyRandom::new(1337).next_int_between_inclusive(min, max.max(min)).ok()
}

/// `BACKPACK_EFFECTS` are the config's `backpackEffects`: (backpack, effect,
/// min ticks, max ticks, amplifier).

/// The attribute modifiers (`*_MODIFIER`): (design, attribute, id, amount, operation).
const BACKPACK_MODIFIERS: [(&str, &str, &str, f64, &str); 9] = [
    ("netherite", "minecraft:armor", "travelersbackpack:netherite_backpack_armor", 4.0, "add_value"),
    ("diamond", "minecraft:armor", "travelersbackpack:diamond_backpack_armor", 3.0, "add_value"),
    ("iron", "minecraft:armor", "travelersbackpack:iron_backpack_armor", 2.0, "add_value"),
    ("gold", "minecraft:armor", "travelersbackpack:gold_backpack_armor", 2.0, "add_value"),
    ("enderman", "minecraft:block_interaction_range", "travelersbackpack:enderman_backpack_reach", 1.0, "add_value"),
    ("enderman", "minecraft:entity_interaction_range", "travelersbackpack:enderman_backpack_reach", 1.0, "add_value"),
    ("warden", "minecraft:max_health", "travelersbackpack:warden_backpack_max_health", 4.0, "add_value"),
    ("fox", "minecraft:movement_speed", "travelersbackpack:fox_movement_speed", 0.1, "add_multiplied_total"),
    ("emerald", "minecraft:luck", "travelersbackpack:emerald_backpack_luck", 1.0, "add_value"),
];

fn design_of(id: &str) -> &str {
    id.strip_prefix("travelersbackpack:").unwrap_or_default()
}

/// Whether `design` has an ability worn (`ITEM_ABILITIES_LIST`) or placed
/// (`BLOCK_ABILITIES_LIST`).
/// `enableBackpackAbilities` and `allowedAbilities`: the design's ability may work.
fn ability_allowed(design: &str) -> bool {
    cfg().enable_backpack_abilities && cfg().allowed_abilities.iter().any(|a| a.strip_prefix("travelersbackpack:") == Some(design))
}

fn ability_worn(design: &str) -> bool {
    ability_allowed(design) && ability(design).is_some_and(|a| a.item)
}

fn ability_placed(design: &str) -> bool {
    ability_allowed(design) && ability(design).is_some_and(|a| a.block)
}

/// The screen's slider (`AbilitySliderButton`): shown for a worn backpack
/// with a worn ability, a placed one with a placed ability; on; its cooldown
/// in seconds.
fn ability_words(game: &Game<'_>, uuid: u128, source: Option<Source>, settings: &Settings) -> [i32; 2] {
    let shown = match source {
        Some(Source::Worn) => worn(game, uuid).is_some_and(|s| ability_worn(design_of(&s.item))),
        Some(Source::Block(pos)) => ability_placed(design_of(&game.native_block(pos))),
        _ => false,
    };
    let flags = i32::from(shown) | i32::from(!settings.ability_off) << 1;
    [flags, (settings.cooldown + 19) / 20]
}

// What each wearer's abilities left on them: the design whose attribute
// modifiers are on, for taking them off again.
thread_local! {
    static MODIFIED: RefCell<HashMap<u128, String>> = RefCell::new(HashMap::new());
    /// Each wearer's ability backpack, for the hooks that have no game
    /// (\`living_change_target\`): its design, while its ability is on.
    static ABILITY_WEARERS: RefCell<HashMap<u128, String>> = RefCell::new(HashMap::new());
}

/// Runs `f` on the player `uuid` as the acting one (their effects, body,
/// food), where the hub keeps them.
fn as_player<T>(game: &mut Game<'_>, uuid: u128, f: impl FnOnce(&mut Game<'_>) -> T) -> Option<T> {
    let conn = game.lan.as_ref()?.hub.guests.iter().find(|g| g.uuid == uuid && g.placed && !g.dead)?.conn;
    let mut host = game.lan.take()?;
    let mut out = None;
    game.as_guest(&mut host, conn, |g| out = Some(f(g)));
    game.lan = Some(host);
    out
}

/// `attributeAbility`: the design's modifiers on, every other's off.
fn set_modifiers(game: &mut Game<'_>, uuid: u128, design: &str) {
    as_player(game, uuid, |g| {
        let Some(p) = g.player.as_mut() else {
            return;
        };
        for (d, attribute, id, amount, operation) in BACKPACK_MODIFIERS {
            let _ = p.set_modifier(attribute, id, amount, operation, d == design);
        }
    });
}

/// `addTimedMobEffect`: put on for its longest, again once at its shortest.
fn timed_effect(game: &mut Game<'_>, uuid: u128, effect: &str, min: i32, max: i32, amplifier: u8) {
    let left = game.native_effects(uuid).and_then(|e| e.into_iter().find(|e| e.id == effect || format!("minecraft:{}", e.id) == effect));
    if left.is_some_and(|e| e.duration < 0 || e.duration > min) {
        return;
    }
    let e = super::native_player_state::NativeEffect { id: effect.to_owned(), amplifier, duration: max, ambient: false, particles: false };
    game.native_add_effect(uuid, &e);
}

/// `Level.canSeeSky`: nothing that collides above.
fn sky_above(game: &Game<'_>, p: [i32; 3]) -> bool {
    (p[1] + 1..p[1] + 200).all(|y| !game.res.facts().get(game.world.block(p[0], y, p[2])).full_collision)
}

fn harmful(id: &str) -> bool {
    horizon_client::brewing::effect(id).is_some_and(|d| d.category == horizon_client_core::potions::Category::Harmful)
}

/// `BackpackWrapper.tick` for every wearer of this level: an ability backpack
/// worn with its ability on works (`abilityTick`), its cooldown counting down
/// 100 ticks every 100; off or taken off, its modifiers go.
fn tick_abilities(game: &mut Game<'_>) {
    remember_players(game);
    // Saved in their own level's pass (its effects and blast are there).
    let here: Vec<u128> = game
        .lan
        .as_ref()
        .map(|h| h.hub.guests.iter().filter(|g| g.dimension == game.dim.id()).map(|g| g.uuid).collect())
        .unwrap_or_default();
    let saves = CREEPER_SAVES.with(|c| {
        let (mine, rest): (Vec<u128>, Vec<u128>) = std::mem::take(&mut *c.borrow_mut()).into_iter().partition(|u| here.contains(u));
        *c.borrow_mut() = rest;
        mine
    });
    for uuid in saves {
        creeper_save(game, uuid);
    }
    let tick = game.world.game_time;
    let dimension = game.dim.id().to_owned();
    let wearers: Vec<(u128, [f64; 3])> = game
        .lan
        .as_ref()
        .map(|h| {
            h.hub.guests.iter().filter(|g| g.placed && !g.dead && g.dimension == dimension).map(|g| (g.uuid, g.position)).collect()
        })
        .unwrap_or_default();
    for (uuid, at) in wearers {
        let stack = worn(game, uuid);
        let design = stack.as_ref().map(|s| design_of(&s.item).to_owned()).unwrap_or_default();
        let mut settings = stack.as_ref().map(item_settings).unwrap_or_default();
        let on = ability_worn(&design) && !settings.ability_off;
        let applied = if on { design.clone() } else { String::new() };
        // On a change, and now and then (a respawn, a join).
        let before = MODIFIED.with(|m| m.borrow().get(&uuid).cloned());
        if before.as_deref() != Some(applied.as_str()) || tick % 100 == 0 {
            set_modifiers(game, uuid, &applied);
            MODIFIED.with(|m| m.borrow_mut().insert(uuid, applied.clone()));
            // The wearer's client: what climbs with them (the spider's walls).
            let _ = game.native_net_send_to_player(uuid, ABILITY_CHANNEL, applied.as_bytes());
        }
        ABILITY_WEARERS.with(|w| {
            let mut w = w.borrow_mut();
            if on {
                w.insert(uuid, design.clone());
            } else {
                w.remove(&uuid);
            }
        });
        let Some(stack) = stack.filter(|_| on) else {
            continue;
        };
        if design == "creeper" && settings.cooldown <= 0 {
            let hp = as_player(game, uuid, |g| g.player.as_ref().map(|p| p.health.health() + p.health.absorption())).flatten();
            let id = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.entity_id);
            if let (Some(hp), Some(id)) = (hp, id) {
                CREEPER_READY.with(|c| c.borrow_mut().insert(id, hp));
            }
        }
        let was = settings.cooldown;
        let just_put_on = before.as_deref() != Some(design.as_str());
        let counts_down = ability_tick(game, uuid, at, &design, &mut settings, just_put_on);
        if counts_down && settings.cooldown > 0 && tick % 100 == 0 {
            settings.cooldown = (settings.cooldown - 100).max(0);
        }
        // Kept at once, its menu open or not (else a ready ability fires every tick).
        if settings.cooldown != was {
            let mut stack = worn(game, uuid).unwrap_or(stack);
            let kept = Settings { cooldown: settings.cooldown, ..item_settings(&stack) };
            set_item_settings(&mut stack, &kept);
            set_worn(game, uuid, Some(&stack));
        }
    }
}

/// `abilityTick` of one worn design; true when its cooldown counts down.
fn ability_tick(game: &mut Game<'_>, uuid: u128, at: [f64; 3], design: &str, settings: &mut Settings, just_put_on: bool) -> bool {
    let tick = game.world.game_time;
    for (d, effect, min, max, amplifier) in &cfg().backpack_effects {
        if d.strip_prefix("travelersbackpack:") != Some(design) {
            continue;
        }
        let (effect, min, max, amplifier) = (effect.as_str(), *min, *max, *amplifier);
        // The squid's night vision only underwater.
        if design == "squid" && effect == "minecraft:night_vision" {
            let feet = at.map(|c| c.floor() as i32);
            let eyes = [feet[0], (at[1] + 1.62).floor() as i32, feet[2]];
            if !game.native_block(eyes).contains("water") && game.native_block_property(eyes, "waterlogged").as_deref() != Some("true") {
                continue;
            }
        }
        timed_effect(game, uuid, effect, min, max, u8::try_from(amplifier).unwrap_or(0));
    }
    let ready = settings.cooldown <= 0;
    match design {
        // Hunger filled, Regeneration for 10 seconds (`cakeAbility`).
        "cake" => {
            if ready {
                // `FoodData.eat(20, 0.1F)`: full, and 4 saturation (up to the food).
                if let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)) {
                    g.food = 20;
                    g.saturation = (g.saturation + 4.0).min(20.0);
                }
                let e = super::native_player_state::NativeEffect {
                    id: "minecraft:regeneration".to_owned(),
                    amplifier: 0,
                    duration: 200,
                    ambient: false,
                    particles: true,
                };
                game.native_add_effect(uuid, &e);
                play_near(game, uuid, "minecraft:entity.generic.eat", "ambient", Some(at), 0.6, 1.0);
                settings.cooldown = cooldown_ticks(design).unwrap_or(0);
            }
            true
        }
        // An egg now and then (`chickenAbility`); put on, it starts waiting.
        "chicken" => {
            if ready {
                settings.cooldown = cooldown_ticks(design).unwrap_or(0);
                if !just_put_on {
                    play_near(game, uuid, "minecraft:entity.chicken.egg", "ambient", Some(at), 1.0, 1.0);
                    game.spawn_item(ItemStack::new("minecraft:egg", 1), [at[0], at[1] + 0.5, at[2]], super::dispenser::new_item_motion(), 10);
                }
            }
            true
        }
        // Its cooldown follows a saved death (`creeperAbility`).
        "creeper" => true,
        // Harmful effects gone (`cowAbility`).
        "cow" => {
            let bad: Vec<String> =
                game.native_effects(uuid).unwrap_or_default().into_iter().map(|e| e.id).filter(|id| harmful(id)).collect();
            if ready && !bad.is_empty() {
                for id in bad {
                    let id = if id.contains(':') { id } else { format!("minecraft:{id}") };
                    game.native_remove_effect(uuid, &id);
                }
                play_near(game, uuid, "minecraft:item.honeycomb.wax_on", "player", Some(at), 1.0, 1.0);
                settings.cooldown = cooldown_ticks(design).unwrap_or(0);
            }
            true
        }
        // No Wither (`witherAbilityTick`).
        "wither" => {
            if game.native_effects(uuid).unwrap_or_default().iter().any(|e| e.id.ends_with("wither")) {
                game.native_remove_effect(uuid, "minecraft:wither");
            }
            false
        }
        // Speed with a monster near (`ocelotAbility`: 6 across, 2 up and down).
        "ocelot" => {
            let near = game.mobs.iter().any(|m| {
                !m.removed
                    && m.alive()
                    && m.hostile()
                    && (m.position.x - at[0]).abs() <= 6.3
                    && (m.position.y - at[1]).abs() <= 3.8
                    && (m.position.z - at[2]).abs() <= 6.3
            });
            if near {
                timed_effect(game, uuid, "minecraft:speed", 20, 30, 0);
            }
            false
        }
        // Rain and water fill its empty tanks (`cactusAbilityEquipped`): 50
        // drops every 5 seconds under each; at 1000 a bucket each.
        "cactus" => {
            if settings.cooldown >= 1000 {
                if let Some((stack, items, tier, k)) = worn_tanks(game, uuid) {
                    let capacity = tank_capacity(tier);
                    let mut tanks = items[k].as_ref().map(tanks_of).unwrap_or_default();
                    for t in &mut tanks {
                        if (t.0.is_empty() || t.0 == "minecraft:water") && t.1 + BUCKET <= capacity {
                            t.0 = "minecraft:water".to_owned();
                            t.1 += BUCKET;
                        }
                    }
                    settings.cooldown = 0;
                    let mut stack = stack;
                    set_item_settings(&mut stack, settings);
                    save_worn_tanks(game, uuid, stack, items, k, &tanks);
                }
            }
            if tick % 100 == 0 {
                let feet = at.map(|c| c.floor() as i32);
                let mut drops = 0;
                if game.native_block(feet).contains("water") {
                    drops += 50;
                }
                if game.world.weather.rain_level.1 > 0.2 && sky_above(game, feet) {
                    drops += 50;
                }
                settings.cooldown += drops;
            }
            false
        }
        _ => false,
    }
}

// ---- abilities heard from the world's events (the original's mixins) ---------------------------

thread_local! {
    /// Players' entity ids, for the hooks that name an entity.
    static PLAYER_IDS: RefCell<HashMap<i32, u128>> = RefCell::new(HashMap::new());
    /// Mobs a player hit (`getLastAttacker`): a ghast or an enderman they hit
    /// may still go for them.
    static HIT_BY: RefCell<HashSet<(u32, u128)>> = RefCell::new(HashSet::new());
    /// A creeper backpack's blast this tick: its wearer, whom it spares.
    static BLASTED: RefCell<Option<u128>> = const { RefCell::new(None) };
    /// The creeper backpack's wearers whose ability is ready: their health
    /// and absorption, by their entity id.
    static CREEPER_READY: RefCell<HashMap<i32, f32>> = RefCell::new(HashMap::new());
    /// Wearers a deadly blow just spared, to be saved this tick.
    static CREEPER_SAVES: RefCell<Vec<u128>> = const { RefCell::new(Vec::new()) };
}

/// The design of the backpack `uuid` wears with its ability on.
fn ability_of(uuid: u128) -> Option<String> {
    ABILITY_WEARERS.with(|w| w.borrow().get(&uuid).cloned())
}

fn wears(uuid: u128, design: &str) -> bool {
    ability_of(uuid).as_deref() == Some(design)
}

fn player_of(entity: i32) -> Option<u128> {
    PLAYER_IDS.with(|p| p.borrow().get(&entity).copied())
}

/// Each tick: who is who, for the hooks.
fn remember_players(game: &Game<'_>) {
    let ids: HashMap<i32, u128> =
        game.lan.as_ref().map(|h| h.hub.guests.iter().map(|g| (g.entity_id, g.uuid)).collect()).unwrap_or_default();
    PLAYER_IDS.with(|p| *p.borrow_mut() = ids);
}

/// `LivingEntity.hurtServer`: the blaze backpack takes no fall (`blazeAbility`
/// resets the fall); the creeper backpack's own blast spares its wearer.
pub(super) fn living_incoming_damage(entity: i32, kind: &str, damage_type: &str, _amount: f32, canceled: &mut bool) {
    if kind != "minecraft:player" {
        return;
    }
    let Some(uuid) = player_of(entity) else {
        return;
    };
    if damage_type == "minecraft:fall" && wears(uuid, "blaze") {
        *canceled = true;
    }
    if damage_type.contains("explosion") && BLASTED.with(|b| *b.borrow() == Some(uuid)) {
        *canceled = true;
    }
}

/// `DeathHandler` (`ALLOW_DEATH`): the creeper backpack's wearer whose
/// ability is ready doesn't die (kept at 1 health) and is saved this tick:
/// the blow counted after armor and effects, as the original.
pub(super) fn creeper_spares(entity: i32) -> bool {
    let Some(uuid) = player_of(entity) else {
        return false;
    };
    if wears(uuid, "creeper") && CREEPER_READY.with(|c| c.borrow_mut().remove(&entity).is_some()) {
        CREEPER_SAVES.with(|c| c.borrow_mut().push(uuid));
        return true;
    }
    false
}

/// `SmallFireballEntityMixin`: a blaze's small fireball does nothing to the
/// blaze backpack's wearer; it is blocked as a shield blocks.
pub(super) fn hears_projectile_impacts(hears: &mut bool) {
    *hears = true;
}

pub(super) fn projectile_impact(game: &mut Game<'_>, kind: &str, entity: Option<u32>, entity_kind: &str, at: [f64; 3], canceled: &mut bool) {
    if kind != "minecraft:small_fireball" || entity_kind != "minecraft:player" {
        return;
    }
    let Some(uuid) = entity.and_then(|e| player_of(e as i32)) else {
        return;
    };
    if wears(uuid, "blaze") {
        *canceled = true;
        play_near(game, uuid, "minecraft:item.shield.block", "player", Some(at), 1.0, 1.0);
    }
}

/// `DeathHandler`: about to die, the creeper backpack's wearer is saved, once
/// a cooldown (`creeperAbility`): at 1 health, its effects gone, Regeneration
/// II, Absorption II and Fire Resistance, a blast around them.
fn creeper_save(game: &mut Game<'_>, uuid: u128) {
    // The blow marked them dead (`LivingEntity.die`): alive again, at 1 health.
    as_player(game, uuid, |g| {
        if let Some(p) = g.player.as_mut() {
            p.health.dead = false;
            let _ = p.set_health(1.0, false);
        }
    });
    if !wears(uuid, "creeper") {
        return;
    }
    let Some(mut stack) = worn(game, uuid) else {
        return;
    };
    let mut settings = item_settings(&stack);
    if settings.cooldown > 0 {
        return;
    }
    game.native_set_effects(uuid, &[]);
    let effect = |id: &str, amplifier: u8, duration: i32| super::native_player_state::NativeEffect {
        id: id.to_owned(),
        amplifier,
        duration,
        ambient: false,
        particles: true,
    };
    game.native_add_effect(uuid, &effect("minecraft:regeneration", 1, 450));
    game.native_add_effect(uuid, &effect("minecraft:absorption", 1, 100));
    game.native_add_effect(uuid, &effect("minecraft:fire_resistance", 0, 400));
    if let Some(at) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.position) {
        let dimension = game.dim.id().to_owned();
        BLASTED.with(|b| *b.borrow_mut() = Some(uuid));
        game.native_explode(&dimension, at, 3.0, false, super::native_explosions::NativeBlast::Keep);
        BLASTED.with(|b| *b.borrow_mut() = None);
        play_near(game, uuid, "minecraft:entity.creeper.primed", "ambient", Some(at), 1.2, 0.5);
    }
    settings.cooldown = cooldown_ticks("creeper").unwrap_or(0);
    set_item_settings(&mut stack, &settings);
    set_worn(game, uuid, Some(&stack));
}

/// `PlayerEntityMixin`'s attack: the bee's sting poisons, the wither's hit
/// withers, the warden's slows.
pub(super) fn hurt_enemy(game: &mut Game<'_>, entity: u32, kind: &str) {
    let uuid = game.native_acting_id();
    if kind == "minecraft:ghast" || kind == "minecraft:enderman" {
        HIT_BY.with(|h| h.borrow_mut().insert((entity, uuid)));
    }
    let Some(design) = ability_of(uuid) else {
        return;
    };
    let (effect, ticks, amplifier) = match design.as_str() {
        "bee" => ("poison", 80, 0),
        "wither" => ("wither", 60, 1),
        "warden" => ("slowness", 40, 1),
        _ => return,
    };
    let Some(i) = game.mobs.iter().position(|m| m.id == entity) else {
        return;
    };
    let Some(id) = super::potions::mob_effect_id(effect) else {
        return;
    };
    if let Some(m) = game.mobs.get_mut(i) {
        m.add_effect(id, ticks, amplifier);
    }
    if design == "bee" {
        let at = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.position);
        play_near(game, uuid, "minecraft:entity.bee.sting", "player", at, 1.0, 1.0);
    }
}

/// `ExperienceOrbEntityMixin`: the lapis backpack, 15% of the time, doubles an orb.
pub(super) fn experience_orb_pickup_listened(listened: &mut bool) {
    *listened = true;
}

pub(super) fn experience_orb_pickup(game: &mut Game<'_>, player: u128, value: &mut u32) {
    use horizon_core::random::RandomSource as _;
    if wears(player, "lapis") && game.world.random.next_float() <= 0.15 {
        *value = value.saturating_mul(2);
    }
}

/// `GhastEntityMixin`, `EndermanEntityMixin`: a ghast leaves the ghast
/// backpack's wearer be unless they hit it; an enderman isn't angered by the
/// pumpkin backpack's wearer looking at it.
pub(super) fn living_change_target(entity: u32, kind: &str, target: Option<u128>, cancel: &mut bool) {
    let Some(uuid) = target else {
        return;
    };
    let design = match kind {
        "minecraft:ghast" => "ghast",
        "minecraft:enderman" => "pumpkin",
        _ => return,
    };
    if wears(uuid, design) && !HIT_BY.with(|h| h.borrow().contains(&(entity, uuid))) {
        *cancel = true;
    }
}

/// `LootTableMixin`: the hay backpack's wearer harvests a ripe crop: each
/// stack 40% of the time doubled; grass, 15% of the time a carrot, potato
/// or beetroot more.
fn hay_drops(game: &mut Game<'_>, context: &NativeLootDrops<'_>, drops: &mut Vec<ItemStack>) {
    use horizon_core::random::RandomSource as _;
    let (Some(player), Some((_, state))) = (context.player, context.block) else {
        return;
    };
    if !wears(player, "hay") {
        return;
    }
    let Some(st) = game.pack.assets.registry.state(state).cloned() else {
        return;
    };
    let age = st.properties.get("age").and_then(|a| a.parse::<i32>().ok());
    if matches!(&*st.name, "minecraft:short_grass" | "minecraft:tall_grass" | "minecraft:fern" | "minecraft:large_fern") {
        if game.world.random.next_float() < 0.15 {
            let crops = ["minecraft:carrot", "minecraft:potato", "minecraft:beetroot"];
            let pick = game.world.random.next_int_bound(3).unwrap_or(0) as usize;
            drops.push(ItemStack::new(crops[pick], 1));
        }
        return;
    }
    // A `CropBlock` at its `MAX_AGE`.
    let ripe = match &*st.name {
        "minecraft:wheat" | "minecraft:carrots" | "minecraft:potatoes" => 7,
        "minecraft:beetroots" => 3,
        _ => return,
    };
    if age != Some(ripe) {
        return;
    }
    for stack in drops.iter_mut() {
        if game.world.random.next_float() < 0.4 {
            stack.count *= 2;
        }
    }
}

// ---- abilities of a placed backpack (`abilityTickBlock`, `TravelersBackpackBlock`) ----------------

thread_local! {
    /// Placed redstone backpacks with their ability on: their signal (15).
    static REDSTONE_ON: RefCell<HashSet<[i32; 3]>> = RefCell::new(HashSet::new());
    /// The redstone backpack's block states (`isSignalSource`).
    static REDSTONE_STATES: RefCell<HashSet<u32>> = RefCell::new(HashSet::new());
    /// Sponge backpacks whose neighbour changed: they soak up water.
    static SPONGE_CHECK: RefCell<HashSet<[i32; 3]>> = RefCell::new(HashSet::new());
}

/// `getSignal`: 15 from a redstone backpack with its ability on.
pub(super) fn block_signal(pos: horizon_core::position::BlockPos, signal: &mut i32) {
    if REDSTONE_ON.with(|r| r.borrow().contains(&[pos.x, pos.y, pos.z])) {
        *signal = 15;
    }
}

/// `isSignalSource`: the redstone backpack.
pub(super) fn is_signal_source(state: horizon_world::registry::BlockStateId, source: &mut bool) {
    if REDSTONE_STATES.with(|r| r.borrow().contains(&state.0)) {
        *source = true;
    }
}

/// `neighborChanged`: a sponge backpack soaks up again.
pub(super) fn neighbor_changed(pos: horizon_core::position::BlockPos) {
    SPONGE_CHECK.with(|s| s.borrow_mut().insert([pos.x, pos.y, pos.z]));
}

/// A placed backpack's ability each tick: the redstone's signal, the
/// sponge's soaking, the cactus's rain, the cooldown counting down.
fn placed_ability_tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut horizon_client::containers::ModBlockEntity) {
    let block = game.native_block(pos);
    let design = design_of(&block).to_owned();
    if !ability_placed(&design) {
        return;
    }
    let mut settings = block_settings(&entity.nbt);
    let on = !settings.ability_off;
    let was = settings.clone();
    if design == "redstone" {
        if REDSTONE_STATES.with(|r| r.borrow().is_empty()) {
            let registry = game.pack.assets.registry;
            let ids: HashSet<u32> = (0..u32::MAX)
                .map_while(|i| registry.state(horizon_world::registry::BlockStateId(i)).map(|s| (i, s.name == block.as_str())))
                .filter(|(_, mine)| *mine)
                .map(|(i, _)| i)
                .collect();
            REDSTONE_STATES.with(|r| *r.borrow_mut() = ids);
        }
        let changed = REDSTONE_ON.with(|r| {
            let mut r = r.borrow_mut();
            if on { r.insert(pos) } else { r.remove(&pos) }
        });
        if changed {
            let dimension = game.dim.id().to_owned();
            game.native_update_signal_neighbors(&dimension, pos);
        }
    }
    if !on {
        return;
    }
    let tier = placed_tier(&entity.nbt, &entity.items);
    let tanks_at = (0..tier.upgrades)
        .find(|&i| entity.items.get(tier.storage + i).and_then(Option::as_ref).is_some_and(|s| &*s.item == TANKS_UPGRADE))
        .map(|i| tier.storage + i);
    match design.as_str() {
        // `tryAbsorbWater`: placed, or a neighbour changed.
        "sponge" => {
            let due = SPONGE_CHECK.with(|s| s.borrow_mut().remove(&pos)) || !matches!(entity.nbt.get(&horizon_world::nbt::JavaString::from("Soaked")), Some(Tag::Byte(1)));
            entity.nbt.insert("Soaked".into(), Tag::Byte(1));
            if due && let Some(k) = tanks_at {
                let capacity = tank_capacity(tier);
                let mut tanks = entity.items[k].as_ref().map(tanks_of).unwrap_or_default();
                if soak_up(game, pos, &mut tanks, capacity)
                    && let Some(u) = entity.items[k].as_mut()
                {
                    set_tanks(u, &tanks);
                }
            }
        }
        // `cactusAbilityBlockEntity`: rain fills its tanks a bucket each at 1000 drops.
        "cactus" => {
            if settings.cooldown >= 1000 {
                if let Some(k) = tanks_at {
                    let capacity = tank_capacity(tier);
                    let mut tanks = entity.items[k].as_ref().map(tanks_of).unwrap_or_default();
                    for t in &mut tanks {
                        if (t.0.is_empty() || t.0 == "minecraft:water") && t.1 + BUCKET <= capacity {
                            t.0 = "minecraft:water".to_owned();
                            t.1 += BUCKET;
                        }
                    }
                    if let Some(u) = entity.items[k].as_mut() {
                        set_tanks(u, &tanks);
                    }
                    settings.cooldown = 0;
                }
            }
            if game.world.game_time % 100 == 0 && game.world.weather.rain_level.1 > 0.2 && sky_above(game, pos) {
                settings.cooldown += 50;
            }
        }
        // The melon's cooldown counts down, a tick a tick (`decreaseCooldown`).
        _ => {
            if settings.cooldown > 0 && cooldown_ticks(&design).is_some() {
                settings.cooldown -= 1;
            }
        }
    }
    if settings != was {
        set_block_settings(&mut entity.nbt, &settings);
    }
}

/// `removeWaterBreadthFirstSearch`: the water around, six blocks out and at
/// most 65, into the tanks a bucket each while they take water; water
/// plants broken. True when any was taken.
fn soak_up(game: &mut Game<'_>, pos: [i32; 3], tanks: &mut Tanks, capacity: i32) -> bool {
    let takes = |t: &(String, i32)| t.0.is_empty() || (t.0 == "minecraft:water" && t.1 < capacity);
    if !tanks.iter().any(takes) {
        return false;
    }
    let dimension = game.dim.id().to_owned();
    let Some(air) = game.native_default_state("minecraft:air") else {
        return false;
    };
    let mut queue = std::collections::VecDeque::from([(pos, 0)]);
    let mut taken = 0;
    while let Some((at, depth)) = queue.pop_front() {
        for d in [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]] {
            let p = [at[0] + d[0], at[1] + d[1], at[2] + d[2]];
            let block = game.native_block(p);
            let waterlogged = game.native_block_property(p, "waterlogged").as_deref() == Some("true");
            let water = &*block == "minecraft:water";
            let plant = matches!(&*block, "minecraft:kelp" | "minecraft:kelp_plant" | "minecraft:seagrass" | "minecraft:tall_seagrass");
            if !(water || waterlogged || plant) {
                continue;
            }
            // A source picked up (`BucketPickup`) fills a tank; flowing water just goes.
            let source = waterlogged || (water && game.native_block_property(p, "level").as_deref() == Some("0"));
            let emptied = if waterlogged { with_property(game, p, "waterlogged", "false") } else { Some(air) };
            let Some(state) = emptied else {
                continue;
            };
            if !game.native_set_block(&dimension, p, state, 3) {
                continue;
            }
            taken += 1;
            if source && let Some(t) = tanks.iter_mut().find(|t| takes(t)) {
                t.0 = "minecraft:water".to_owned();
                t.1 = (t.1 + BUCKET).min(capacity);
            }
            if depth < 6 {
                queue.push_back((p, depth + 1));
            }
        }
        if taken > 64 {
            break;
        }
    }
    taken > 0
}

/// `TravelersBackpackBlock.playerWillDestroy`: the melon backpack broken with
/// its ability ready drops up to two slices, and waits its cooldown.
fn melon_broken(game: &mut Game<'_>, pos: [i32; 3]) {
    use horizon_core::random::RandomSource as _;
    if design_of(&game.native_block(pos)) != "melon" {
        return;
    }
    let mut ready = false;
    game.native_with_mod_block_entity(pos, |m| {
        let mut settings = block_settings(&m.nbt);
        if !settings.ability_off && settings.cooldown <= 0 {
            ready = true;
            settings.cooldown = cooldown_ticks("melon").unwrap_or(0);
            set_block_settings(&mut m.nbt, &settings);
        }
    });
    if ready {
        let count = game.world.random.next_int_bound(3).unwrap_or(0) as u32;
        if count > 0 {
            let at = pos.map(|c| f64::from(c) + 0.5);
            game.spawn_item(ItemStack::new("minecraft:melon_slice", count), at, super::dispenser::new_item_motion(), 10);
        }
    }
}


// ---- the tanks on the HUD (`HudOverlay`) -------------------------------------------------------

/// `HudOverlay.renderOverlay`: the worn backpack's tanks at the bottom right
/// (20 and 30 in from the corner), each filled as full as it is, the one the
/// hose in hand draws from marked. Drawn before the hotbar.
pub(super) fn hud_tanks(game: &Game<'_>, layer: &str, batch: &mut horizon_client::hud::HudBatch) {
    GUI_HEIGHT.with(|h| h.set(batch.height));
    if !layer.ends_with("hotbar") || game.spectator() {
        return;
    }
    let me = game.own_profile_name();
    let Some(Worn { tanks: true, fluids, .. }) = WEARING.with(|w| w.borrow().get(&me).cloned()) else {
        return;
    };
    if !cfg().overlay_enabled {
        return;
    }
    let [off_x, off_y] = cfg().overlay_offset;
    let (x, y) = (batch.width - off_x as f32, batch.height - off_y as f32);
    let selected = game
        .inventory
        .get(game.selected)
        .cloned()
        .flatten()
        .filter(|s| &*s.item == HOSE)
        .map_or(0, |s| hose_settings(&s).1);
    if let Some(levels) = fluids {
        // The right tank, then the left (`drawGuiTank`, 21 by 8).
        for ((code, fill), tx) in [(levels[1], x + 1.0), (levels[0], x - 11.0)] {
            if code <= 0 || fill <= 0.0 {
                continue;
            }
            // A potion in its own colour.
            let colour = 0xFF00_0000 | tank_code_colour(code);
            let level = (21.0 * fill.min(1.0)).round().max(1.0);
            let top = y + 21.0 - level;
            if code == 2 && game.native_sprite(batch, "block/lava_still", [0.0, 16.0 - level.min(16.0), 8.0, level.min(16.0)], [tx, top, 8.0, level]) {
                continue;
            }
            game.native_fill(batch, [tx, top, 8.0, level], colour);
        }
    }
    let overlay = "travelersbackpack:gui/overlay";
    game.native_sprite(batch, overlay, [if selected == 2 { 0.0 } else { 10.0 }, 0.0, 10.0, 23.0], [x, y, 10.0, 23.0]);
    game.native_sprite(batch, overlay, [if selected == 1 { 0.0 } else { 10.0 }, 0.0, 10.0, 23.0], [x - 12.0, y, 10.0, 23.0]);
}

// ---- the void upgrade's trash bin and a click on a tank (`VoidWidget`, `TankWidget`) -----------

/// A carried container clicked on tank `side` (`TankActions.fillTank`): a
/// bucket of a fluid or a potion pours in; an empty bucket or bottle fills
/// from it. A bottle from a stack goes into the inventory.
fn fill_tank(game: &mut Game<'_>, uuid: u128, side: usize) {
    let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
        return;
    };
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    let Some((mut items, mut carried)) =
        game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
    else {
        return;
    };
    let (Some(held), Some(i)) = (carried.clone(), tanks_slot(&items, tier)) else {
        return;
    };
    let k = tier.storage + i;
    let Some(upgrade) = items[k].as_ref() else {
        return;
    };
    let mut tanks = tanks_of(upgrade);
    let capacity = tank_capacity(tier);
    let tank = &mut tanks[side];
    let mut extra = None;
    let sound;
    match container_of(&held) {
        Some(Container::Pour { fluid, amount, empty }) if held.count == 1 => {
            if !(tank.0.is_empty() || tank.0 == fluid) || tank.1 + amount > capacity {
                return;
            }
            sound = if fluid == "minecraft:lava" { "minecraft:item.bucket.empty_lava" } else { "minecraft:item.bucket.empty" };
            tank.0 = fluid;
            tank.1 += amount;
            carried = (!empty.is_empty()).then(|| ItemStack::new(empty, 1));
        }
        Some(Container::Fill { amount, bottle }) => {
            let Some(full) = filled(&tank.0, bottle).filter(|_| tank.1 >= amount) else {
                return;
            };
            sound = if bottle {
                "minecraft:item.bottle.fill"
            } else if tank.0 == "minecraft:lava" {
                "minecraft:item.bucket.fill_lava"
            } else {
                "minecraft:item.bucket.fill"
            };
            tank.1 -= amount;
            if tank.1 == 0 {
                tank.0.clear();
            }
            if held.count > 1 {
                carried = Some(ItemStack { count: held.count - 1, ..held });
                extra = Some(full);
            } else {
                carried = Some(full);
            }
        }
        _ => return,
    }
    if let Some(u) = items[k].as_mut() {
        set_tanks(u, &tanks);
    }
    if let Some(full) = extra
        && let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid))
    {
        match free_slot(&g.inventory) {
            Some(slot) => g.inventory[slot] = Some(full),
            None => {
                let at = g.position;
                game.spawn_item(full, at, super::dispenser::new_item_motion(), 10);
            }
        }
    }
    with(|s| s.extras.insert(uuid, (tier, items[tier.storage..].to_vec())));
    if let Some(host) = game.lan.as_mut() {
        horizon_client::lan::containers::travelersbackpack_menu::set_items(&mut host.hub, conn, items, carried);
    }
    let at = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.position);
    play_near(game, uuid, sound, "block", at, 1.0, 1.0);
    resend_data(game, uuid);
}

/// A player's trash bin (`VoidWidget.hoveringTrashBin`): on, the menu slots
/// and tanks they chose to void.
#[derive(Clone, Debug, Default)]
struct Trash {
    window: i32,
    slots: Vec<usize>,
    tanks: [bool; 2],
}

thread_local! {
    static TRASH: RefCell<HashMap<u128, Trash>> = RefCell::new(HashMap::new());
    /// On the client: whether the trash bin is on, for the screen's clicks.
    static TRASH_SHOWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// On the client: menu slots clicked with the trash bin, to send (slot, right).
    static TRASH_CLICKS: RefCell<Vec<(u16, bool)>> = const { RefCell::new(Vec::new()) };
}
const TRASH_CHANNEL: &str = "travelersbackpack:trash";

/// The trash bin's words in the data slots: on, the tanks; the storage's
/// slots chosen, 16 a word; the inventory's 36.
fn trash_words(uuid: u128, storage: usize, menu: usize) -> Vec<i32> {
    let t = TRASH.with(|t| t.borrow().get(&uuid).cloned());
    let words = storage.div_ceil(16);
    let mut out = vec![0i32; 1 + words + 3];
    if let Some(t) = t {
        out[0] = 1 | i32::from(t.tanks[0]) << 1 | i32::from(t.tanks[1]) << 2;
        for &s in &t.slots {
            if s < storage {
                out[1 + s / 16] |= 1 << (s % 16);
            } else if let Some(k) = s.checked_sub(menu).filter(|&k| k < 36) {
                out[1 + words + k / 16] |= 1 << (k % 16);
            }
        }
    }
    out
}

/// The bin clicked: on; off again, what was chosen goes (`SET_STACK` empty).
fn trash_bin(game: &mut Game<'_>, uuid: u128) {
    let Some((_, _, window)) = with(|s| s.open.get(&uuid).cloned()) else {
        return;
    };
    let was = TRASH.with(|t| t.borrow_mut().remove(&uuid)).filter(|t| t.window == window);
    match was {
        None => {
            TRASH.with(|t| t.borrow_mut().insert(uuid, Trash { window, ..Trash::default() }));
        }
        Some(t) => void_chosen(game, uuid, &t),
    }
    resend_data(game, uuid);
}

fn void_chosen(game: &mut Game<'_>, uuid: u128, t: &Trash) {
    let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
        return;
    };
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    let Some((mut items, carried)) =
        game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
    else {
        return;
    };
    let menu = tier.menu_slots();
    for &s in &t.slots {
        if s < tier.storage {
            items[s] = None;
        } else if let Some(k) = s.checked_sub(menu).filter(|&k| k < 36) {
            let slot = if k < 27 { 9 + k } else { k - 27 };
            if let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid))
                && let Some(x) = g.inventory.get_mut(slot)
            {
                *x = None;
            }
        }
    }
    if t.tanks.iter().any(|&b| b)
        && let Some(i) = tanks_slot(&items, tier)
        && let Some(u) = items[tier.storage + i].as_mut()
    {
        let mut tanks = tanks_of(u);
        for (side, chosen) in t.tanks.iter().enumerate() {
            if *chosen {
                tanks[side] = (String::new(), 0);
            }
        }
        set_tanks(u, &tanks);
    }
    with(|s| s.extras.insert(uuid, (tier, items[tier.storage..].to_vec())));
    if let Some(host) = game.lan.as_mut() {
        horizon_client::lan::containers::travelersbackpack_menu::set_items(&mut host.hub, conn, items, carried);
    }
}

/// A slot clicked with the trash bin: left chooses it (if it holds
/// something), right lets it go (`selectSlotsIndex`).
fn trash_slot(game: &mut Game<'_>, uuid: u128, slot: usize, right: bool) {
    let Some((_, _, window)) = with(|s| s.open.get(&uuid).cloned()) else {
        return;
    };
    let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
        return;
    };
    let menu = tier.menu_slots();
    let holds = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).is_some_and(|g| {
        if slot < tier.storage {
            horizon_client::lan::containers::travelersbackpack_menu::items(&game.lan.as_ref().unwrap().hub, g.conn)
                .is_some_and(|(items, _)| items.get(slot).is_some_and(Option::is_some))
        } else if let Some(k) = slot.checked_sub(menu).filter(|&k| k < 36) {
            g.inventory.get(if k < 27 { 9 + k } else { k - 27 }).is_some_and(Option::is_some)
        } else {
            false
        }
    });
    TRASH.with(|t| {
        if let Some(t) = t.borrow_mut().get_mut(&uuid).filter(|t| t.window == window) {
            if right {
                t.slots.retain(|&s| s != slot);
            } else if holds && !t.slots.contains(&slot) {
                t.slots.push(slot);
            }
        }
    });
    resend_data(game, uuid);
}

/// A tank clicked: with the trash bin, chosen or let go; else a container
/// carried is poured or filled there.
fn tank_clicked(game: &mut Game<'_>, uuid: u128, side: usize) {
    let window = with(|s| s.open.get(&uuid).map(|o| o.2));
    let chosen = TRASH.with(|t| {
        t.borrow_mut().get_mut(&uuid).filter(|t| Some(t.window) == window).map(|t| {
            t.tanks[side] = !t.tanks[side];
        })
    });
    if chosen.is_some() {
        resend_data(game, uuid);
    } else {
        fill_tank(game, uuid, side);
    }
}

/// On the client: a press on a slot while the trash bin is on chooses it
/// instead of clicking it.
pub(super) fn trash_press(screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if !TRASH_SHOWN.with(std::cell::Cell::get) || button > 1 {
        return;
    }
    let Some((layout, _)) = screen.menu.and_then(Layout::of) else {
        return;
    };
    let at = [screen.mouse[0] - screen.rect[0], screen.mouse[1] - screen.rect[1]];
    let over = |[x, y]: [f32; 2]| at[0] >= x - 1.0 && at[0] < x + 17.0 && at[1] >= y - 1.0 && at[1] < y + 17.0;
    let tier = layout.tier;
    let mut slot = (0..tier.storage).find(|&i| layout.storage_shown(i) && over(layout.storage_slot(i)));
    if slot.is_none() {
        let (ix, iy) = (8.0 + layout.additions(), layout.slots_height() + 7.0 + 25.0);
        slot = (0..36).find_map(|k| {
            let (row, col) = (k / 9, k % 9);
            let y = if row < 3 { iy + 18.0 * row as f32 } else { iy + 58.0 };
            over([ix + 18.0 * col as f32, y]).then_some(tier.menu_slots() + k)
        });
    }
    if let Some(s) = slot {
        *canceled = true;
        TRASH_CLICKS.with(|c| c.borrow_mut().push((s as u16, button == 1)));
    }
}

/// On the client, each tick: the chosen slots to the server.
pub(super) fn send_trash_clicks(game: &mut Game<'_>) {
    send_radial_clicks(game);
    inventory_icon_tick(game);
    show_messages(game);
    write_mod_menu_files(game);
    if SCROLLED.with(|s| s.replace(false)) {
        super::native_hooks::travelersbackpack_screen::relayout(game);
    }
    for (slot, right) in TRASH_CLICKS.with(|c| std::mem::take(&mut *c.borrow_mut())) {
        let [lo, hi] = slot.to_le_bytes();
        let _ = game.native_net_send_to_server(TRASH_CHANNEL, &[lo, hi, u8::from(right)]);
    }
}

/// `VoidWidget`: the bin in the void tab (lit while on), the chosen slots
/// and tanks in red, the bin at the pointer.
fn draw_trash(
    game: &Game<'_>,
    layout: &Layout,
    live: &Live,
    batch: &mut horizon_client::hud::HudBatch,
    [ox, oy]: [f32; 2],
    mouse: [f32; 2],
) {
    const RED: u32 = 0x7FC9_1616;
    if layout.info.settings {
        return;
    }
    for (tab, [tx, ty, _, _]) in layout.tab_rects() {
        if tab == tb::Tab::Void {
            if live.trash {
                game.native_fill(batch, [ox + tx + 25.0, oy + ty + 4.0, 16.0, 16.0], 0x80FF_FFFF);
            }
            game.native_item(batch, "travelersbackpack:void_upgrade", [ox + tx + 25.0, oy + ty + 4.0], 16.0);
        }
    }
    if !live.trash {
        return;
    }
    for (s, chosen) in live.trash_storage.iter().enumerate() {
        if *chosen && layout.storage_shown(s) {
            let [x, y] = layout.storage_slot(s);
            game.native_fill(batch, [ox + x, oy + y, 16.0, 16.0], RED);
        }
    }
    let (ix, iy) = (8.0 + layout.additions(), layout.slots_height() + 7.0 + 25.0);
    for (k, chosen) in live.trash_inventory.iter().enumerate() {
        if *chosen {
            let (row, col) = (k / 9, k % 9);
            let y = if row < 3 { iy + 18.0 * row as f32 } else { iy + 58.0 };
            game.native_fill(batch, [ox + ix + 18.0 * col as f32, oy + y, 16.0, 16.0], RED);
        }
    }
    if layout.tanks() {
        for (side, [x, y, w, h]) in layout.tank_columns().into_iter().enumerate() {
            if live.trash_tanks[side] || inside([x, y, w, h], mouse) {
                let colour = if live.trash_tanks[side] { RED } else { 0x80FF_FFFF };
                game.native_fill(batch, [ox + x + 1.0, oy + y + 1.0, w - 2.0, h - 2.0], colour);
            }
        }
    }
    game.native_item(batch, "travelersbackpack:void_upgrade", [ox + mouse[0] - 1.0, oy + mouse[1] - 1.0], 16.0);
}

thread_local! {
    /// On the client: the open backpack screen's menu and data, for what is
    /// drawn over its items.
    static LAST_SCREEN: RefCell<Option<(String, Vec<i32>)>> = const { RefCell::new(None) };
}

/// Over the open backpack screen, items included: the trash bin's marks.
pub(super) fn screen_render_post(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
    GUI_HEIGHT.with(|h| h.set(batch.height));
    if radial_open(game) {
        draw_radial(game, batch, screen);
        return;
    }
    draw_inventory_extras(game, batch, screen);
    let Some(menu) = screen.menu else {
        return;
    };
    let Some((m, data)) = LAST_SCREEN.with(|l| l.borrow().clone()) else {
        return;
    };
    if m != menu {
        return;
    }
    let Some((layout, _)) = Layout::of(menu) else {
        return;
    };
    let live = Live::of(&layout, &data);
    let origin = [screen.rect[0], screen.rect[1]];
    let mouse = [screen.mouse[0] - origin[0], screen.mouse[1] - origin[1]];
    draw_trash(game, &layout, &live, batch, origin, mouse);
    // `renderFluidWarningTooltip`: three seconds after a tanks upgrade too full was refused.
    let warning = FLUID_WARNING.with(|w| w.borrow().clone()).filter(|(at, _)| at.elapsed().as_secs_f32() < 3.0);
    if let Some((_, over)) = warning {
        let mut lines = vec![
            Text::legacy(&tr(game, "screen.travelersbackpack.cant_apply_upgrade")),
            Text::legacy(&tr(game, "screen.travelersbackpack.too_much_fluid")),
        ];
        for l in over {
            let mut p = l.split('\t');
            let (fluid, amount, capacity) = (p.next().unwrap_or_default(), p.next().unwrap_or_default(), p.next().unwrap_or_default());
            lines.push(Text::legacy(&format!("§c{} {amount}/{capacity}mB", fluid_name(game, fluid))));
        }
        game.native_gui_tooltip(batch, &lines, screen.mouse);
    }
}

/// A tanks upgrade refused for its fluid: the client shows why.
const FLUID_WARNING_CHANNEL: &str = "travelersbackpack:fluid_warning";

thread_local! {
    /// On the client: when the warning came, and its fluids (`fluid\tamount\tcapacity`).
    static FLUID_WARNING: RefCell<Option<(std::time::Instant, Vec<String>)>> = const { RefCell::new(None) };
}

// ---- the filters' tag selector (`FilterUpgradeWidgetBase`, `FilterSettingsBase.filterTags`) ----

/// Where a filter keeps its chosen tags (`FILTER_TAGS`): its custom data.
const FILTER_TAGS_KEY: &str = "TravelersBackpackFilterTags";
/// The server tells a client each open filter's tags: chosen and addable.
const TAGS_CHANNEL: &str = "travelersbackpack:filter_tags";

/// `hasTagSelector`: every filter but the feeding upgrade's.
fn has_tag_selector(tab: tb::Tab) -> bool {
    matches!(tab, tb::Tab::Void | tb::Tab::Pickup | tb::Tab::Magnet)
}

/// `isTagSelector`: its second button on "tag".
fn tag_mode(stack: &ItemStack) -> bool {
    has_tag_selector(tb::Tab::of(Some(&stack.item))) && filter_settings(stack)[1] == 2
}

fn filter_tags(stack: &ItemStack) -> Vec<String> {
    match custom_data(stack).get(&horizon_world::nbt::JavaString::from(FILTER_TAGS_KEY)) {
        Some(Tag::List(list)) => list
            .iter()
            .filter_map(|t| match t {
                Tag::String(s) => s.to_string().ok(),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn set_filter_tags(stack: &mut ItemStack, tags: &[String]) {
    let mut data = custom_data(stack);
    let key = horizon_world::nbt::JavaString::from(FILTER_TAGS_KEY);
    if tags.is_empty() {
        data.remove(&key);
    } else {
        data.insert(key, Tag::List(tags.iter().map(|t| Tag::String(t.as_str().into())).collect()));
    }
    set_custom_data(stack, data);
}

/// A filter's chosen tags, and the tags of the item in its slot not chosen
/// yet (`getAddableTags`); whether that slot holds an item (`isAdding`).
fn tag_lists(game: &Game<'_>, upgrade: &ItemStack, item: Option<&ItemStack>) -> (bool, Vec<String>, Vec<String>) {
    let chosen = filter_tags(upgrade);
    let mut addable: Vec<String> = item
        .as_ref()
        .map(|it| {
            game.items()
                .item_tags
                .iter()
                .filter(|(name, set)| set.contains(&*it.item) && !chosen.contains(name))
                .map(|(name, _)| name.clone())
                .collect()
        })
        .unwrap_or_default();
    addable.sort();
    (item.is_some(), chosen, addable)
}

/// A tag clicked (`addTag`, `removeTag`): with an item in the slot, the
/// selected addable tag is chosen; without, the selected chosen one goes.
fn filter_tag_clicked(game: &mut Game<'_>, uuid: u128, i: usize, k: usize) {
    let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
        return;
    };
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    let Some((items, _)) =
        game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
    else {
        return;
    };
    let Some(upgrade) = items.get(tier.storage + i).cloned().flatten() else {
        return;
    };
    // While the menu is open the selector's item is in its slot of the menu.
    let item = items.get(tier.slots() + i * tb::TAB_SLOTS).cloned().flatten();
    let (adding, mut chosen, addable) = tag_lists(game, &upgrade, item.as_ref());
    if adding {
        let Some(tag) = addable.get(k) else {
            return;
        };
        chosen.push(tag.clone());
    } else if k < chosen.len() {
        chosen.remove(k);
    } else {
        return;
    }
    edit_upgrade(game, uuid, i, |u| set_filter_tags(u, &chosen));
}

/// The open menu closed and opened again, its slots laid out anew.
fn reopen_menu(game: &mut Game<'_>, uuid: u128) {
    let Some((_, _, window)) = with(|s| s.open.get(&uuid).cloned()) else {
        return;
    };
    let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
        return;
    };
    if let Some(host) = game.lan.as_mut()
        && horizon_client::lan::containers::travelersbackpack_menu::close(&mut host.hub, conn, window)
    {
        with(|s| s.reopen.insert(uuid));
    }
}

thread_local! {
    /// What each player's client was last told of its filters' tags.
    static TAGS_SENT: RefCell<HashMap<u128, (i32, String)>> = RefCell::new(HashMap::new());
}

/// Each tick: a client whose open menu's tag selectors changed is told them,
/// a line a filter: `<slot>\t<adding>\t<chosen,...>\t<addable,...>`.
fn send_tag_lists(game: &mut Game<'_>) {
    let open: Vec<(u128, i32)> = with(|s| s.open.iter().map(|(u, o)| (*u, o.2)).collect());
    for (uuid, window) in open {
        let Some(tier) = with(|s| s.extras.get(&uuid).map(|e| e.0)) else {
            continue;
        };
        let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) else {
            continue;
        };
        let Some((items, _)) =
            game.lan.as_ref().and_then(|h| horizon_client::lan::containers::travelersbackpack_menu::items(&h.hub, conn))
        else {
            continue;
        };
        let mut message = String::new();
        for i in 0..tier.upgrades {
            let Some(u) = items.get(tier.storage + i).and_then(Option::as_ref).filter(|u| tag_mode(u)) else {
                continue;
            };
            let item = items.get(tier.slots() + i * tb::TAB_SLOTS).and_then(Option::as_ref);
            let (adding, chosen, addable) = tag_lists(game, u, item);
            message.push_str(&format!("{i}\t{}\t{}\t{}\n", u8::from(adding), chosen.join(","), addable.join(",")));
        }
        let same = TAGS_SENT.with(|t| t.borrow().get(&uuid).is_some_and(|(w, m)| *w == window && *m == message));
        if !same {
            let _ = game.native_net_send_to_player(uuid, TAGS_CHANNEL, message.as_bytes());
            TAGS_SENT.with(|t| t.borrow_mut().insert(uuid, (window, message)));
        }
    }
}

thread_local! {
    /// On the client: each upgrade slot's tag selector (adding, chosen, addable).
    static TAG_LISTS: RefCell<HashMap<usize, (bool, Vec<String>, Vec<String>)>> = RefCell::new(HashMap::new());
    /// On the client: the tag each selector points at (`selectedTagIndex`).
    static TAG_SELECTED: RefCell<HashMap<usize, usize>> = RefCell::new(HashMap::new());
}

fn tag_lists_received(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let split = |s: &str| s.split(',').filter(|t| !t.is_empty()).map(str::to_owned).collect::<Vec<_>>();
    let lists: HashMap<usize, (bool, Vec<String>, Vec<String>)> = text
        .lines()
        .filter_map(|l| {
            let mut p = l.split('\t');
            let i = p.next()?.parse().ok()?;
            let adding = p.next()? == "1";
            Some((i, (adding, split(p.next().unwrap_or_default()), split(p.next().unwrap_or_default()))))
        })
        .collect();
    // A list that changed starts at its top (`updateAddableTags`), else keeps within it.
    TAG_SELECTED.with(|sel| {
        let mut sel = sel.borrow_mut();
        TAG_LISTS.with(|old| {
            let old = old.borrow();
            for (i, now) in &lists {
                let shown = if now.0 { &now.2 } else { &now.1 };
                let before = old.get(i).map(|o| if o.0 { o.2.clone() } else { o.1.clone() });
                let k = sel.entry(*i).or_insert(0);
                if before.as_ref().is_some_and(|b| b.len() != shown.len() && now.0) || old.get(i).is_some_and(|o| o.0 != now.0) {
                    *k = 0;
                }
                *k = (*k).min(shown.len().saturating_sub(1));
            }
        });
    });
    TAG_LISTS.with(|t| *t.borrow_mut() = lists);
}

/// What upgrade slot `i`'s selector lists: the item's tags to add, or the chosen ones.
fn shown_tags(i: usize) -> (bool, Vec<String>) {
    TAG_LISTS.with(|t| t.borrow().get(&i).map(|(a, c, ad)| (*a, if *a { ad.clone() } else { c.clone() })).unwrap_or_default())
}

fn selected_tag(i: usize) -> usize {
    TAG_SELECTED.with(|s| s.borrow().get(&i).copied().unwrap_or(0))
}

/// `getTooltip`: the list in grey, the selected one marked green (adding) or
/// red (removing), what a click does; or how to start.
fn tag_tooltip(game: &Game<'_>, i: usize) -> Vec<String> {
    let (adding, list) = shown_tags(i);
    if list.is_empty() {
        return vec![format!("§8{}", tr(game, "screen.travelersbackpack.filter_tag_empty"))];
    }
    let k = selected_tag(i).min(list.len() - 1);
    let mut lines: Vec<String> = list
        .iter()
        .enumerate()
        .map(|(n, t)| if n == k { format!("{}> {t}", if adding { "§a" } else { "§c" }) } else { format!("§7{t}") })
        .collect();
    let hint = if adding { "screen.travelersbackpack.filter_tag_add" } else { "screen.travelersbackpack.filter_tag_remove" };
    lines.push(format!("§9{}", tr(game, hint)));
    lines
}

/// `getTrimmedText`: cut to `max` wide with "...".
fn trim_to(game: &Game<'_>, text: &str, max: f32) -> String {
    if game.native_text_width(text) <= max {
        return text.to_owned();
    }
    let room = max - game.native_text_width("...");
    let mut cut = text.to_owned();
    while !cut.is_empty() && game.native_text_width(&cut) > room {
        cut.pop();
    }
    format!("{cut}...")
}

/// `mouseScrolled` over a tag list: the selection moves along it.
/// `ToolsScreen.mouseScrolled`: with `allowHotbarScrolling`, the wheel picks
/// the hotbar slot while the tools' menu is open (at the next tick).
pub(super) fn radial_scroll(game: &Game<'_>, scroll: [f64; 2], canceled: &mut bool) {
    if !radial_open(game) || !cfg().tools_allow_hotbar_scrolling || scroll[1] == 0.0 {
        return;
    }
    *canceled = true;
    let step = if scroll[1] > 0.0 { 8 } else { 1 };
    RADIAL_SCROLL.with(|r| r.set(Some((game.selected + step) % 9)));
}

pub(super) fn tag_scroll(screen: &horizon_client::mod_api::NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    let Some(menu) = screen.menu else {
        return;
    };
    let at = [screen.mouse[0] - screen.rect[0], screen.mouse[1] - screen.rect[1]];
    let data = LAST_SCREEN.with(|l| l.borrow().clone()).filter(|(m, _)| m == menu).map(|(_, d)| d).unwrap_or_default();
    let Some(&(Widget::Tags(i), _)) = widgets(menu, &data).iter().find(|(w, r)| matches!(w, Widget::Tags(_)) && inside(*r, at)) else {
        storage_scroll(menu, at, scroll, canceled);
        return;
    };
    *canceled = true;
    let len = shown_tags(i).1.len();
    TAG_SELECTED.with(|s| {
        let mut s = s.borrow_mut();
        let k = s.entry(i).or_insert(0);
        if scroll[1] < 0.0 && *k + 1 < len {
            *k += 1;
        } else if scroll[1] > 0.0 && *k > 0 {
            *k -= 1;
        }
    });
}

// ---- the storage's scroll on a short window (`AbstractBackpackScreen.updateDimensions`) ----

thread_local! {
    /// On the client: the GUI's height, from the last frame drawn.
    static GUI_HEIGHT: std::cell::Cell<f32> = const { std::cell::Cell::new(0.0) };
    /// On the client: the open backpack's menu and its first row shown.
    static SCROLL: RefCell<Option<(String, usize)>> = const { RefCell::new(None) };
    /// On the client: the rows shown moved; the screen's slots follow at the tick.
    static SCROLLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `InventoryScroll.mouseScrolled`: a row a notch, over the storage or its bar.
fn storage_scroll(menu: &str, at: [f32; 2], scroll: [f64; 2], canceled: &mut bool) {
    let Some((layout, _)) = Layout::of(menu) else {
        return;
    };
    let Some([bx, by, bw, bh]) = layout.scroll_bar() else {
        return;
    };
    let area = [7.0 + layout.tank_offset(), by, bx + bw - 7.0 - layout.tank_offset(), bh];
    if !inside(area, at) || scroll[1] == 0.0 {
        return;
    }
    *canceled = true;
    let max = layout.tier.rows() - layout.visible;
    let now = layout.scroll;
    let next = if scroll[1] < 0.0 { (now + 1).min(max) } else { now.saturating_sub(1) };
    if next != now {
        SCROLL.with(|s| *s.borrow_mut() = Some((menu.to_owned(), next)));
        SCROLLED.with(|s| s.set(true));
    }
}

// ---- the mod's row in Options > Mods, with its keys (`KeybindHandler`) -------------------------

/// The keys as the original registers them: name, its label's key, default.
const KEYS: [(&str, &str); 8] = [
    ("inventory", "key.keyboard.b"),
    ("sort", ""),
    ("ability", ""),
    ("cycle_tool", "key.keyboard.z"),
    ("toggle_upgrade_0", ""),
    ("toggle_upgrade_1", ""),
    ("toggle_upgrade_2", ""),
    ("toggle_upgrade_3", ""),
];

/// The mod's row: its keys listed first on its page, where the player sets them.
pub(super) fn mods(mods: &mut Vec<super::native_hooks::NativeMod>) {
    let label = |name: &str| {
        let key = format!("key.travelersbackpack.{name}");
        horizon_client::lang::get_or(&key, &key)
    };
    mods.push(super::native_hooks::NativeMod {
        id: MOD_ID.into(),
        title: format!("{} {VERSION}", horizon_client::lang::get_or("itemGroup.travelersbackpack", "Traveler's Backpack")),
        description: if horizon_client::lang::code().starts_with("ru") {
            "Уникальные и улучшаемые рюкзаки с настройкой. Порт Traveler's Backpack.".into()
        } else {
            "Unique and upgradeable backpacks with customisation. A port of Traveler's Backpack.".into()
        },
        keys: KEYS
            .iter()
            .map(|(name, default)| [(*name).to_owned(), label(name), if default.is_empty() { "key.keyboard.unknown" } else { default }.to_owned()])
            .collect(),
    });
}

// ---- the tools' radial menu (`ToolsScreen`, `RadialToolsOverlay`) ----

/// The client asks for the worn backpack's tools; the server answers on it:
/// the backpack (for the middle), its tanks, then each tool slot, a line each.
const TOOLS_CHANNEL: &str = "travelersbackpack:tools";
/// `SWAP_TOOL`: a tool slot (`ADD_NEW`: the first free one) and the hand
/// (0 the main one, 1 the other).
const TOOL_SWAP_CHANNEL: &str = "travelersbackpack:swap_tool";
/// `RadialToolsOverlay.ADD_NEW`.
const ADD_NEW: i32 = -999;
/// `ICON_SIZE`, `ITEM_RING_RADIUS`, `DEADZONE_RADIUS`.
const RADIAL_ICON: f32 = 16.0;
const RADIAL_RING: f32 = 42.0;
const RADIAL_DEADZONE: f32 = 30.0;
const BACKPACK_TANK: &str = "travelersbackpack:backpack_tank";

/// The open menu, on the client.
struct Radial {
    opened: std::time::Instant,
    /// The hose's menu (`hoseMenu`), else the tools'.
    hose: bool,
    /// What the server sent: the backpack, its tanks (fluid, amount; the
    /// capacity), its tool slots.
    backpack: Option<ItemStack>,
    tanks: [(String, i32); 2],
    capacity: i32,
    tools: Vec<Option<ItemStack>>,
    /// What the pointer is over (`hoveredResult`), and a click swapped already.
    hovered: i32,
    clicked: bool,
}

thread_local! {
    static RADIAL: RefCell<Option<Radial>> = const { RefCell::new(None) };
    /// Clicks in the menu, sent at the next tick.
    static RADIAL_CLICKS: RefCell<Vec<(i32, u8)>> = const { RefCell::new(Vec::new()) };
    /// A hotbar slot the wheel picked in the menu.
    static RADIAL_SCROLL: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// `ToolsScreen.createHoseMenu`: suck, drink, the right tank, the left, spill.
fn hose_menu() -> Vec<Option<ItemStack>> {
    let hose = |mode: i32| {
        let mut h = ItemStack::new(HOSE, 1);
        set_hose_settings(&mut h, mode, 1);
        Some(h)
    };
    vec![hose(SUCK_MODE), hose(DRINK_MODE), Some(ItemStack::new(BACKPACK_TANK, 1)), Some(ItemStack::new(BACKPACK_TANK, 1)), hose(SPILL_MODE)]
}

/// `KeybindHandler.SWAP_TOOL` pressed while playing in a worn backpack: the
/// menu opens (the mouse freed in the middle, the game not paused).
fn open_radial(game: &mut Game<'_>) {
    let me = game.own_profile_name();
    if game.spectator() || !WEARING.with(|w| w.borrow().contains_key(&me)) {
        return;
    }
    let hose = game.inventory.get(game.selected).cloned().flatten().is_some_and(|s| &*s.item == HOSE);
    if !game.native_mod_screen_open(MOD_ID) {
        return;
    }
    let tools = if hose { hose_menu() } else { Vec::new() };
    RADIAL.with(|r| {
        *r.borrow_mut() = Some(Radial {
            opened: std::time::Instant::now(),
            hose,
            backpack: None,
            tanks: Default::default(),
            capacity: 0,
            tools,
            hovered: -1,
            clicked: false,
        });
    });
    let _ = game.native_net_send_to_server(TOOLS_CHANNEL, &[]);
}

/// Whether this mod's menu is the open screen; a menu closed some other way
/// (Escape) is forgotten.
fn radial_open(game: &Game<'_>) -> bool {
    let open = game.native_mod_screen() == Some(MOD_ID);
    if !open {
        RADIAL.with(|r| r.borrow_mut().take());
    }
    open && RADIAL.with(|r| r.borrow().is_some())
}

/// The key let go (`!isKeyDown(SWAP_TOOL)`): the hose's choice is made, or
/// the hand swaps with the tool under the pointer (`swapOnClose`), and the
/// menu closes.
fn radial_released(game: &mut Game<'_>) {
    if !radial_open(game) {
        return;
    }
    let Some(r) = RADIAL.with(|r| r.borrow_mut().take()) else {
        return;
    };
    game.native_mod_screen_close(MOD_ID);
    if r.hose {
        // `selectHoseAction`.
        let (channel, value, key) = match r.hovered {
            0 => (HOSE_MODE_CHANNEL, SUCK_MODE, hose_mode_key(SUCK_MODE)),
            1 => (HOSE_MODE_CHANNEL, DRINK_MODE, hose_mode_key(DRINK_MODE)),
            4 => (HOSE_MODE_CHANNEL, SPILL_MODE, hose_mode_key(SPILL_MODE)),
            2 => (HOSE_TANK_CHANNEL, 2, hose_tank_key(2)),
            3 => (HOSE_TANK_CHANNEL, 1, hose_tank_key(1)),
            _ => return,
        };
        let _ = game.native_net_send_to_server(channel, &[value as u8]);
        let text = Text::literal(game.translate(key, key));
        game.native_client_action_bar(text, false);
    } else if r.hovered != -1 && !r.clicked && cfg().tools_swap_on_close {
        send_swap(game, r.hovered, 0);
    }
}

fn send_swap(game: &mut Game<'_>, slot: i32, button: u8) {
    let [lo, hi] = (slot as i16).to_le_bytes();
    let _ = game.native_net_send_to_server(TOOL_SWAP_CHANNEL, &[lo, hi, button]);
}

/// `ToolsScreen.mouseClicked`: a tool or the plus clicked swaps with the main
/// hand (the right button: the other), and the release then doesn't.
pub(super) fn radial_press(game: &Game<'_>, button: i32, canceled: &mut bool) {
    if !radial_open(game) {
        return;
    }
    *canceled = true;
    let hovered = RADIAL.with(|r| r.borrow().as_ref().filter(|r| !r.hose).map_or(-1, |r| r.hovered));
    if hovered == -1 {
        return;
    }
    RADIAL.with(|r| {
        if let Some(r) = r.borrow_mut().as_mut() {
            r.clicked = true;
        }
    });
    RADIAL_CLICKS.with(|c| c.borrow_mut().push((hovered, u8::from(button == 1))));
}

/// Each client tick: the menu's clicks to the server.
fn send_radial_clicks(game: &mut Game<'_>) {
    if let Some(slot) = RADIAL_SCROLL.with(std::cell::Cell::take) {
        game.selected = slot;
    }
    for (slot, button) in RADIAL_CLICKS.with(|c| std::mem::take(&mut *c.borrow_mut())) {
        send_swap(game, slot, button);
    }
}

/// The server's answer: the backpack, its tanks and its tools.
fn tools_received(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let parse = |line: &str| -> Option<ItemStack> {
        let (tag, _) = horizon_world::nbt::parse_snbt(line).ok()?;
        horizon_items::item_nbt::item_from_nbt(&tag)
    };
    let mut lines = text.split('\n');
    let backpack = lines.next().and_then(parse);
    let mut t = lines.next().unwrap_or_default().split('\t');
    let mut tank = || (t.next().unwrap_or_default().to_owned(), t.next().and_then(|n| n.parse().ok()).unwrap_or(0));
    let tanks = [tank(), tank()];
    let capacity = t.next().and_then(|n| n.parse().ok()).unwrap_or(0);
    let tools: Vec<Option<ItemStack>> = lines.map(parse).collect();
    RADIAL.with(|r| {
        if let Some(r) = r.borrow_mut().as_mut() {
            r.backpack = backpack;
            r.tanks = tanks;
            r.capacity = capacity;
            if !r.hose {
                r.tools = tools;
            }
        }
    });
}

/// The worn backpack's tool slots, from its contents.
fn worn_tool_range(tier: tb::Tier) -> std::ops::Range<usize> {
    let base = tier.storage + tier.upgrades;
    base..base + tier.tools
}

/// Tells `uuid`'s client its worn backpack's tools (an answer to the menu).
fn send_tools(game: &mut Game<'_>, uuid: u128) {
    let Some(stack) = worn(game, uuid) else {
        return;
    };
    let tier = stack_tier(&stack);
    let items = contents(&stack, tier.slots());
    let snbt = |s: &ItemStack| horizon_world::snbt_text::to_snbt(&horizon_items::item_nbt::item_to_nbt(s));
    // The middle's backpack: its look, not its contents.
    let mut look = stack.clone();
    look.data_mut().container = Vec::new();
    look.data_mut().wire = Default::default();
    look.tidy();
    let tanks = tanks_slot(&items, tier)
        .and_then(|i| items[tier.storage + i].as_ref())
        .map(tanks_of)
        .unwrap_or_default();
    let mut out = vec![
        snbt(&look),
        format!("{}\t{}\t{}\t{}\t{}", tanks[0].0, tanks[0].1, tanks[1].0, tanks[1].1, tank_capacity(tier)),
    ];
    for k in worn_tool_range(tier) {
        out.push(items.get(k).cloned().flatten().map(|s| snbt(&s)).unwrap_or_default());
    }
    let _ = game.native_net_send_to_player(uuid, TOOLS_CHANNEL, out.join("\n").as_bytes());
}

/// `ServerActions.swapTool`: the hand's stack and tool slot `slot` change
/// places (`ADD_NEW`: the hand's goes into the first free one); a stack
/// the tool slots don't take stays. Not while the worn backpack's menu is open.
fn swap_tool(game: &mut Game<'_>, uuid: u128, slot: i32, button: u8) {
    if !cfg().allow_tool_swapping {
        return;
    }
    if worn_menu_open(uuid) {
        return;
    }
    let Some(mut stack) = worn(game, uuid) else {
        return;
    };
    let tier = stack_tier(&stack);
    let mut items = contents(&stack, tier.slots());
    let range = worn_tool_range(tier);
    let Some(g) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)) else {
        return;
    };
    let (selected, position) = (g.selected, g.position);
    let hand = if button == 0 { g.inventory.get(selected).cloned().flatten() } else { g.offhand.clone() };
    if hand.as_ref().is_some_and(|h| !fits_tool_slot(game, h)) {
        return;
    }
    let new_hand = if slot == ADD_NEW {
        let Some(held) = hand else {
            return;
        };
        let Some(k) = range.clone().find(|&k| items[k].is_none()) else {
            return;
        };
        items[k] = Some(held);
        None
    } else {
        let Some(k) = usize::try_from(slot).ok().map(|s| range.start + s).filter(|k| range.contains(k)) else {
            return;
        };
        std::mem::replace(&mut items[k], hand)
    };
    set_contents(&mut stack, items);
    if !set_worn(game, uuid, Some(&stack)) {
        return;
    }
    if let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)) {
        if button == 0 {
            if let Some(s) = g.inventory.get_mut(selected) {
                *s = new_hand;
            }
        } else {
            g.offhand = new_hand;
        }
    }
    advance(game, uuid, "swap_tools");
    tell_worn(game, uuid);
    let at = [position[0], position[1] + 0.5, position[2]];
    play_near(game, uuid, "minecraft:item.armor.equip_leather", "block", Some(at), 1.0, 1.0);
    send_tools(game, uuid);
}

/// `RadialToolsOverlay.buildSegToSlot`: the filled slots in order, and the
/// first free one when the hand's stack may go in (`displayEmptySlots` off).
fn radial_segments(tools: &[Option<ItemStack>], can_add: bool) -> (Vec<usize>, Option<usize>) {
    if cfg().tools_display_empty_slots {
        return ((0..tools.len()).collect(), None);
    }
    let mut segments: Vec<usize> = (0..tools.len()).filter(|&i| tools[i].is_some()).collect();
    let plus = can_add.then(|| (0..tools.len()).find(|&i| tools[i].is_none())).flatten();
    if let Some(p) = plus {
        let at = segments.iter().position(|&s| s > p).unwrap_or(segments.len());
        segments.insert(at, p);
    }
    (segments, plus)
}

/// `getHoveredIndex`: the segment the pointer points at, outside the middle.
fn radial_hovered(centre: [f32; 2], mouse: [f32; 2], segments: usize) -> Option<usize> {
    let (dx, dy) = (mouse[0] - centre[0], mouse[1] - centre[1]);
    if segments == 0 || dx * dx + dy * dy < RADIAL_DEADZONE * RADIAL_DEADZONE {
        return None;
    }
    let two_pi = std::f32::consts::TAU;
    let rel = (dy.atan2(dx) - std::f32::consts::PI * 1.5).rem_euclid(two_pi);
    let step = two_pi / segments as f32;
    Some(((rel + step / 2.0) / step).floor() as usize % segments)
}

/// `RadialToolsOverlay.renderRadial`: the ring's background growing in, the
/// tools (or the hose's choices) round it, the one pointed at lit with its
/// name, in the middle the backpack or what a click does.
fn draw_radial(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
    let Some((opened, hose, backpack, tanks, capacity, tools)) =
        RADIAL.with(|r| r.borrow().as_ref().map(|r| (r.opened, r.hose, r.backpack.clone(), r.tanks.clone(), r.capacity, r.tools.clone())))
    else {
        return;
    };
    // `getScaledWindow`: the middle, moved by the config's offsets (pixels of a 1920 by 1080 window).
    let [off_x, off_y] = cfg().tools_offset;
    let centre = [
        (batch.width / 2.0 + off_x as f32 / 1920.0 * batch.width).round(),
        (batch.height / 2.0 + off_y as f32 / 1080.0 * batch.height).round(),
    ];
    let t = (opened.elapsed().as_secs_f32() * 1000.0 / 180.0).clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    // The ring, 256 square, scaled up as it opens.
    if let Some(r) = game.gui.atlas.sprites.get("travelersbackpack:gui/tools_overlay").copied() {
        let size = 256.0 * t;
        let alpha = (t.min(cfg().tools_opacity) * 255.0) as u8;
        batch.rect_uv(centre[0] - size / 2.0, centre[1] - size / 2.0, size, size, r, horizon_client::hud::rgba(255, 255, 255, alpha));
    }
    let main = game.inventory.get(game.selected).cloned().flatten();
    let held = main.or_else(|| game.stack_in(super::shield::OFFHAND_SLOT).cloned());
    let hand_empty = held.is_none();
    let can_add = !hose
        && held.as_ref().is_some_and(|h| fits_tool_slot(game, h))
        && tools.iter().filter(|s| s.is_some()).count() < tools.len();
    let (segments, plus) = if hose { ((0..tools.len()).collect(), None) } else { radial_segments(&tools, can_add) };
    let hovered_seg = radial_hovered(centre, screen.mouse, segments.len());
    let mut hovered = -1;
    let mut hovered_plus = false;
    let mut hovered_empty = false;
    if let Some(seg) = hovered_seg {
        let slot = segments[seg];
        if tools[slot].is_some() {
            hovered = slot as i32;
        } else if cfg().tools_display_empty_slots {
            // An empty slot shown: the hand's stack goes into it, or it says so.
            hovered_plus = can_add;
            hovered_empty = !can_add;
            hovered = if can_add { slot as i32 } else { -1 };
        } else if can_add && plus == Some(slot) {
            hovered_plus = true;
            hovered = ADD_NEW;
        }
    }
    RADIAL.with(|r| {
        if let Some(r) = r.borrow_mut().as_mut() {
            r.hovered = hovered;
        }
    });
    let step = std::f32::consts::TAU / segments.len().max(1) as f32;
    for (seg, &slot) in segments.iter().enumerate() {
        let angle = -std::f32::consts::FRAC_PI_2 + seg as f32 * step;
        let x = centre[0] + (angle.cos() * RADIAL_RING * t).floor() - RADIAL_ICON / 2.0;
        let y = centre[1] + (angle.sin() * RADIAL_RING * t).floor() - RADIAL_ICON / 2.0;
        let stack = tools[slot].as_ref();
        // Each icon grows in with the ring (the overlay's pose scaled by it).
        let start = batch.vertices.len();
        if hovered_seg == Some(seg) && (stack.is_some() || can_add || hovered_empty) {
            game.native_fill(batch, [x - 2.0, y - 2.0, RADIAL_ICON + 4.0, RADIAL_ICON + 4.0], 0x80FF_FFFF);
        }
        match stack {
            Some(s) => game.draw_item(batch, s, x, y),
            None => {
                let sign = if can_add { "+" } else { "-" };
                let w = game.native_text_width(sign);
                game.native_text(batch, sign, [x + (RADIAL_ICON - w) / 2.0 + 0.5, y + 5.0], 0x00FF_FFFF, false);
            }
        }
        batch.scale_since(start, x + RADIAL_ICON / 2.0, y + RADIAL_ICON / 2.0, t, t);
    }
    let mouse = screen.mouse;
    let mut centred_item = true;
    if hovered_plus {
        let lines = vec![(Text::literal(tr(game, "screen.travelersbackpack.add")), u32::MAX)];
        game.draw_tooltip_lines(batch, &lines, mouse[0], mouse[1]);
    } else if let Some(stack) = usize::try_from(hovered).ok().and_then(|h| tools.get(h)).and_then(Option::as_ref) {
        // `showTooltip` off: the name alone, but all of the hose's.
        let mut lines = game.stack_tooltip(stack);
        if !hose && !cfg().tools_show_tooltip {
            lines.truncate(1);
        }
        if &*stack.item == BACKPACK_TANK {
            // The tank's fluid (`TankWidget.getTankTooltip`): the right one, then the left.
            let (fluid, amount) = if hovered == 2 { &tanks[1] } else { &tanks[0] };
            if fluid.is_empty() || *amount == 0 {
                lines.push((Text::literal(tr(game, "screen.travelersbackpack.empty")), u32::MAX));
            } else {
                lines.push((Text::literal(fluid_name(game, fluid)), u32::MAX));
                lines.push((Text::legacy(&format!("§7{}/{} mB", mb(*amount), mb(capacity))), u32::MAX));
            }
        } else {
            centred_item = false;
            let key = if hand_empty { "screen.travelersbackpack.take" } else { "screen.travelersbackpack.swap" };
            radial_centre_text(game, batch, &tr(game, key), centre);
        }
        game.draw_tooltip_lines(batch, &lines, mouse[0], mouse[1]);
    }
    if hovered_empty {
        centred_item = false;
        radial_centre_text(game, batch, &tr(game, "screen.travelersbackpack.empty"), centre);
    }
    // `renderBackpackIconInCenter`.
    if centred_item && cfg().tools_render_backpack_icon_in_center && let Some(b) = backpack.as_ref() {
        let start = batch.vertices.len();
        game.draw_item(batch, b, centre[0] - 8.0, centre[1] - 8.0);
        batch.scale_since(start, centre[0], centre[1], t, t);
    }
}

fn radial_centre_text(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, text: &str, centre: [f32; 2]) {
    let w = game.native_text_width(text);
    game.native_text(batch, text, [centre[0] - (w / 2.0).floor(), centre[1] - 4.0], 0x00FF_FFFF, true);
}

// ---- the death message, dropped backpacks, Mod Menu ----

/// `ClientboundSendMessagePacket`: where a dead player's backpack went
/// (`<dropped 0/1>\t<x>\t<y>\t<z>`).
const MESSAGE_CHANNEL: &str = "travelersbackpack:send_message";

thread_local! {
    /// On the client: messages for the chat, shown at the next tick.
    static PENDING_MESSAGES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static MOD_MENU_WRITTEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The chat line, unless the client's config turns it off.
fn show_messages(game: &mut Game<'_>) {
    for text in PENDING_MESSAGES.with(|m| std::mem::take(&mut *m.borrow_mut())) {
        if !cfg().send_backpack_coordinates_message {
            continue;
        }
        let mut parts = text.split('\t');
        let dropped = parts.next() == Some("1");
        let coords: Vec<String> = parts.map(str::to_owned).collect();
        let key = if dropped { "information.travelersbackpack.backpack_drop" } else { "information.travelersbackpack.backpack_coords" };
        let line = format_args(&tr(game, key), &coords);
        game.system_message(Text::literal(line));
    }
}

/// `BackpackItemEntity` / `ItemEntityMixin`: a dropped backpack never
/// despawns (`neverDespawn`), doesn't burn (`fireResistant`), takes no
/// damage (`invulnerableBackpack`, `explosionResistant`) and, fallen out of
/// the world, floats back up and stays there (`voidProtection`).
fn protect_dropped(game: &mut Game<'_>) {
    let c = cfg();
    let min_y = f64::from(game.world.shape.min_y);
    for item in game.dropped.iter_mut().filter(|d| tb::is_backpack(&d.stack.item)) {
        if c.never_despawn {
            item.age = 0;
        }
        if c.fire_resistant || c.invulnerable_backpack {
            item.fire_ticks = 0;
        }
        if c.invulnerable_backpack || c.explosion_resistant || c.fire_resistant {
            item.health = item.health.max(1_000_000);
        }
        if c.void_protection && !item.no_gravity {
            if item.position.y < min_y + 1.0 {
                // Floating up as in a bubble column.
                item.velocity.y = (item.velocity.y + 0.06).min(0.7);
                item.needs_sync = true;
            } else if item.velocity.y > 0.0 && item.position.y < min_y + 2.0 {
                item.no_gravity = true;
                item.velocity = horizon_core::geometry::Vec3::new(0.0, 0.0, 0.0);
                item.needs_sync = true;
            }
        }
    }
}

/// Mod Menu's icon and metadata (`fabric.mod.json`) in the mod's folder,
/// each written when it differs, once a run.
fn write_mod_menu_files(game: &Game<'_>) {
    if MOD_MENU_WRITTEN.with(|w| w.replace(true)) {
        return;
    }
    const ICON: &[u8] = horizon_client::native_data!("travelersbackpack_native", "icon.png");
    let dir = game.native_data_dir(MOD_ID);
    let _ = std::fs::create_dir_all(&dir);
    let json = format!(
        r#"{{
  "schema": 1,
  "name": "Traveler's Backpack",
  "version": "{VERSION}",
  "description": "Unique and upgradeable backpacks with customization and more!",
  "description_ru": "Уникальные и улучшаемые рюкзаки с настройкой. Порт Traveler's Backpack.",
  "authors": ["Tiviacz1337"],
  "contributors": ["DispatchedToHeaven (Horizon port)"],
  "license": "LGPL-3.0",
  "links": {{
    "homepage": "https://www.curseforge.com/minecraft/mc-mods/travelers-backpack-fabric",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/travelersbackpack",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": [],
  "config_screen": false
}}
"#
    );
    for (name, bytes) in [("icon.png", ICON), ("modmenu.json", json.as_bytes())] {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(bytes) {
            let _ = std::fs::write(&path, bytes);
        }
    }
}

/// `TierConfig.tankCapacity`: each tank holds the config's amount a row of
/// storage (a bucket by default), two rows more on the wider screen.
fn tank_capacity(tier: tb::Tier) -> i32 {
    let i = ["leather", "iron", "gold", "diamond", "netherite"].iter().position(|n| *n == tier.name).unwrap_or(0);
    let rows = tier.rows() + if tier.extended() { 2 } else { 0 };
    (rows as i32).saturating_mul(cfg().tank_capacity[i]).saturating_mul(FLUID_UNIT)
}

/// `PlayerMixin`'s slowness: every 100 ticks, a player (not in creative)
/// carrying more backpacks than `maxNumberOfBackpacks` (the worn one, the
/// inventory's, the other hand's) is slowed, a level per
/// `slownessPerExcessedBackpack` past it, at most 10, for 200 ticks.
fn backpack_slowness(game: &mut Game<'_>) {
    let c = cfg();
    if !c.too_many_backpacks_slowness || game.world.game_time % 100 != 0 {
        return;
    }
    let players: Vec<(u128, usize)> = game
        .lan
        .as_ref()
        .map(|h| {
            h.hub
                .guests
                .iter()
                .filter(|g| g.placed && !g.dead && g.game_mode != 1)
                .map(|g| {
                    let carried = g.inventory.iter().flatten().chain(g.offhand.iter()).filter(|s| tb::is_backpack(&s.item)).count();
                    (g.uuid, carried)
                })
                .collect()
        })
        .unwrap_or_default();
    for (uuid, carried) in players {
        let count = carried + usize::from(worn(game, uuid).is_some());
        let max = usize::try_from(c.max_number_of_backpacks).unwrap_or(3);
        if count == 0 || count <= max {
            continue;
        }
        let levels = (((count - max) as f64 * c.slowness_per_excessed_backpack).ceil() as i32).clamp(1, 10);
        let e = super::native_player_state::NativeEffect {
            id: "minecraft:slowness".to_owned(),
            amplifier: (levels - 1) as u8,
            duration: 200,
            ambient: false,
            particles: false,
        };
        game.native_add_effect(uuid, &e);
    }
}

/// Whether `state` is a sleeping bag's (for its respawn point).
pub(super) fn is_bag_state(game: &Game<'_>, state: horizon_world::registry::BlockStateId) -> bool {
    is_sleeping_bag(&game.res.facts().get(state).name)
}

/// `TravelersBackpackItem.overrideStackedOnOther` and
/// `overrideOtherStackedOnMe`: a right click (not in creative) with a backpack
/// held on an item of the inventory, or with an item held on a backpack
/// there, puts the item into the backpack's storage, as much as fits; the
/// click does nothing else.
pub(super) fn stack_into_backpack(
    game: &Game<'_>,
    host: &mut super::lan::LanHost,
    conn: u32,
    p: &mut horizon_protocol::packets::play::ContainerClick,
) {
    if p.input != 0 || p.button != 1 {
        return;
    }
    let Some(g) = host.hub.guest(conn) else {
        return;
    };
    if g.game_mode == 1 {
        return;
    }
    let Ok(clicked) = usize::try_from(p.slot) else {
        return;
    };
    // The inventory slot the click is on: the player's own window (main 9..36,
    // hotbar 36..45, the other hand 45), or any menu's inventory rows after its own.
    let size = if p.container == 0 { 9 } else { g.menu.as_ref().filter(|m| m.window == p.container).map_or(usize::MAX, |m| m.size) };
    let slot = if p.container == 0 {
        match clicked {
            9..=35 => Some(clicked),
            36..=44 => Some(clicked - 36),
            45 => Some(shield_slot()),
            _ => None,
        }
    } else {
        clicked.checked_sub(size).and_then(|k| match k {
            0..=26 => Some(9 + k),
            27..=35 => Some(k - 27),
            _ => None,
        })
    };
    let Some(slot) = slot else {
        return;
    };
    let in_slot = if slot == shield_slot() { g.offhand.clone() } else { g.inventory.get(slot).cloned().flatten() };
    let Some(target) = in_slot else {
        return;
    };
    let Some(carried) = g.carried.clone() else {
        return;
    };
    // Which is the backpack, which the item going in.
    let (backpack_held, mut backpack, item) = if tb::is_backpack(&carried.item) && carried.count == 1 && fits_storage(game, &target) {
        (true, carried, target)
    } else if tb::is_backpack(&target.item) && target.count == 1 && fits_storage(game, &carried) {
        (false, target, carried)
    } else {
        return;
    };
    let tier = stack_tier(&backpack);
    let mut items = contents(&backpack, tier.slots());
    let left = insert_stacked(game, &mut items[..tier.storage], item.clone());
    let moved = item.count - left.as_ref().map_or(0, |l| l.count);
    // The click is spent either way.
    (p.slot, p.button, p.input) = (-999, 3 << 2, 5);
    if moved <= 0 {
        return;
    }
    set_contents(&mut backpack, items);
    let uuid = {
        let Some(g) = host.hub.guest_mut(conn) else {
            return;
        };
        let rest = left.filter(|l| l.count > 0);
        let (held, in_slot) = if backpack_held { (Some(backpack), rest) } else { (rest, Some(backpack)) };
        g.carried = held;
        if slot == shield_slot() {
            g.offhand = in_slot;
        } else if let Some(s) = g.inventory.get_mut(slot) {
            *s = in_slot;
        }
        g.uuid
    };
    STACKED_SOUNDS.with(|s| s.borrow_mut().push(uuid));
}

/// The other hand's place in a guest's inventory, as `stack_into_backpack` counts.
fn shield_slot() -> usize {
    super::shield::OFFHAND_SLOT
}

thread_local! {
    /// Players who put something into a backpack, for `BUNDLE_INSERT` at the tick.
    static STACKED_SOUNDS: RefCell<Vec<u128>> = const { RefCell::new(Vec::new()) };
}

/// `playInsertSound`.
fn stacked_sounds(game: &mut Game<'_>) {
    for uuid in STACKED_SOUNDS.with(|s| std::mem::take(&mut *s.borrow_mut())) {
        let at = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.position);
        if let Some(at) = at {
            let pitch = 0.8 + (game.world.game_time % 5) as f32 * 0.1;
            play_near(game, uuid, "minecraft:item.bundle.insert", "player", Some(at), 0.8, pitch);
        }
    }
}

// ---- the player's inventory (`ScreenRenderHandler`, `BackpackIconCommands`) ----

thread_local! {
    /// `showBackpackIconInInventory` as `/tb_client hide|show` set it this run.
    static ICON_SHOWN: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    /// A left click on the worn backpack's icon, sent at the tick.
    static ICON_OPEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The inventory slot under the pointer, and when it was seen.
    static HOVERED_SLOT: std::cell::Cell<Option<(usize, std::time::Instant)>> = const { std::cell::Cell::new(None) };
    /// A chat line from the inventory (`hide_icon_info`), shown at the tick.
    static ICON_INFO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn icon_shown() -> bool {
    ICON_SHOWN.with(std::cell::Cell::get).unwrap_or(cfg().show_backpack_icon_in_inventory)
}

/// The worn backpack's icon's place on the inventory screen (`leftPos + 77`, `topPos + 44`).
fn icon_rect(screen: &horizon_client::mod_api::NativeScreenView<'_>) -> [f32; 4] {
    [screen.rect[0] + 77.0, screen.rect[1] + 44.0, 16.0, 16.0]
}

/// The player's own inventory screen (not a container, not creative).
fn own_inventory(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>) -> bool {
    screen.id == "inventory" && screen.menu.is_none() && !game.creative_inventory_open() && !game.creative_tab_open()
}

/// The worn backpack as its icon shows it, from what the server told.
fn worn_icon_stack(game: &Game<'_>) -> Option<ItemStack> {
    let me = game.own_profile_name();
    let w = WEARING.with(|w| w.borrow().get(&me).cloned())?;
    let mut stack = ItemStack::new(&w.backpack, 1);
    let mut look = Compound::new();
    look.insert("flags".into(), Tag::List(vec![Tag::Byte(i8::from(w.tanks))]));
    look.insert("strings".into(), Tag::List(vec![Tag::String(w.bag.as_str().into())]));
    stack.data_mut().set_other("minecraft:custom_model_data", Some(Tag::Compound(look)));
    stack.data_mut().wire = Default::default();
    stack.tidy();
    set_item_dye(&mut stack, w.dye);
    Some(stack)
}

/// `renderAboveContents`: over any inventory screen (not in creative), a
/// backpack held marks the items it can take (`-`), an item held marks the
/// backpacks it can go into (`+`), with "Right click to add" over them; on
/// the player's own inventory, the worn backpack's icon above the armour.
fn draw_inventory_extras(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
    if screen.id != "inventory" || game.creative_inventory_open() || game.creative_tab_open() {
        return;
    }
    let (ox, oy) = game.inventory_origin(batch);
    let mouse = screen.mouse;
    let hovered = game.slot_at(batch, mouse[0], mouse[1]);
    HOVERED_SLOT.with(|h| h.set(hovered.map(|i| (i, std::time::Instant::now()))));
    let add = || vec![(Text::literal(format!("§e{}", tr(game, "screen.travelersbackpack.add_to_backpack"))), u32::MAX)];
    if !game.creative {
        if let Some(carried) = game.carried.as_ref() {
            for i in game.slots().into_iter().filter(|&i| i < 36) {
                let Some(stack) = game.inventory.get(i).cloned().flatten() else {
                    continue;
                };
                let (sx, sy) = game.slot_position(i);
                let (x, y) = (ox + sx, oy + sy);
                if tb::is_backpack(&carried.item) {
                    if fits_storage(game, &stack) {
                        game.native_text(batch, "-", [x + 2.0, y - 1.0], 0x00FF_FF55, false);
                        if hovered == Some(i) {
                            game.draw_tooltip_lines(batch, &add(), mouse[0], mouse[1]);
                        }
                    }
                } else if fits_storage(game, carried) && tb::is_backpack(&stack.item) {
                    game.native_text(batch, "+", [x + 9.0, y + 8.0], 0x00FF_FF55, false);
                    if hovered == Some(i) {
                        game.draw_tooltip_lines(batch, &add(), mouse[0], mouse[1]);
                    }
                }
            }
        }
    }
    if !own_inventory(game, screen) || !icon_shown() {
        return;
    }
    let Some(stack) = worn_icon_stack(game) else {
        return;
    };
    let [x, y, w, h] = icon_rect(screen);
    let over = inside([x, y, w, h], mouse);
    if over {
        game.draw_slot_highlight(batch, false, x, y);
    }
    game.draw_item(batch, &stack, x, y);
    if over {
        game.draw_slot_highlight(batch, true, x, y);
        let key = game.native_mod_key(MOD_ID, "inventory", "key.keyboard.b");
        let name = key.rsplit('.').next().unwrap_or("b").to_uppercase();
        let mut lines = vec![
            (Text::legacy(&format_args(&tr(game, "screen.travelersbackpack.open_inventory"), &[name])), u32::MAX),
            (Text::legacy(&tr(game, "screen.travelersbackpack.unequip_tip")), u32::MAX),
            (Text::legacy(&tr(game, "screen.travelersbackpack.hide_icon")), u32::MAX),
        ];
        lines.extend(game.stack_tooltip(&stack).into_iter().skip(1));
        game.draw_tooltip_lines(batch, &lines, mouse[0], mouse[1]);
    }
}

/// A click on the worn backpack's icon: the left button opens it, shift and
/// the right one tells how to hide the icon.
pub(super) fn inventory_icon_press(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if *canceled || !own_inventory(game, screen) || !icon_shown() || worn_icon_stack(game).is_none() {
        return;
    }
    if !inside(icon_rect(screen), screen.mouse) {
        return;
    }
    if button == 0 {
        *canceled = true;
        ICON_OPEN.with(|o| o.set(true));
    } else if button == 1 && game.native_has_shift_down() {
        *canceled = true;
        ICON_INFO.with(|o| o.set(true));
    }
}

/// At the client tick: the icon's click done.
fn inventory_icon_tick(game: &mut Game<'_>) {
    if ICON_OPEN.with(|o| o.replace(false)) {
        let _ = game.native_net_send_to_server(OPEN_CHANNEL, &[]);
    }
    if ICON_INFO.with(|o| o.replace(false)) {
        let line = tr(game, "screen.travelersbackpack.hide_icon_info");
        game.system_message(Text::legacy(&line));
    }
}

/// `/tb_client`'s name, for the chat box.
pub(super) fn client_commands(commands: &mut Vec<&'static str>) {
    commands.push("tb_client");
}

/// `/tb_client hide` and `/tb_client show`: the worn backpack's icon in the
/// inventory, kept in the client config.
pub(super) fn client_command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    let mut words = command.split_whitespace();
    if words.next() != Some("tb_client") {
        return;
    }
    let show = match words.next() {
        Some("hide") => false,
        Some("show") => true,
        _ => return,
    };
    *ran = true;
    ICON_SHOWN.with(|s| s.set(Some(show)));
    if let Ok(dir) = Game::native_config_dir() {
        let path = dir.join("travelersbackpack-client.toml");
        if let Ok(text) = std::fs::read_to_string(&path) {
            let changed: Vec<String> = text
                .lines()
                .map(|l| {
                    if l.trim_start().starts_with("showBackpackIconInInventory") {
                        let indent = &l[..l.len() - l.trim_start().len()];
                        format!("{indent}showBackpackIconInInventory = {show}")
                    } else {
                        l.to_owned()
                    }
                })
                .collect();
            let _ = std::fs::write(&path, changed.join("\n") + "\n");
        }
    }
    if !show {
        let line = tr(game, "screen.travelersbackpack.hidden_icon_info");
        game.system_message(Text::legacy(&line));
    }
}

// ---- `/tb` (`AccessCommand`, `ClearCommand`, `RestoreCommand`, `UnpackCommand`) and the backups (`BackpackManager`) ----

/// The commands, for operators (`COMMANDS_GAMEMASTER`: level 2).
pub(super) fn register_commands(commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    use horizon_client::mod_api::NativeCommand;
    for usage in [
        "tb access <pos:block_pos>",
        "tb access <target:player>",
        "tb remove [player:player]",
        "tb clear [player:player]",
        "tb restore <target:player> <backpack_id:word>",
        "tb unpack <pos:block_pos>",
        "tb unpack <target:player>",
    ] {
        commands.push(NativeCommand::op(usage, 2));
    }
}

/// A coordinate of the command: a number, `~` or `~n` from the player's.
fn command_coord(word: &str, own: f64) -> Option<i32> {
    match word.strip_prefix('~') {
        Some("") => Some(own.floor() as i32),
        Some(rest) => rest.parse::<f64>().ok().map(|d| (own + d).floor() as i32),
        None => word.parse::<f64>().ok().map(|n| n.floor() as i32),
    }
}

/// A player named in the command (`@s` and `@p` are the one running it).
fn command_player(game: &Game<'_>, word: Option<&str>, own: u128) -> Option<(u128, String)> {
    let host = game.lan.as_ref()?;
    match word {
        None | Some("@s") | Some("@p") => host.hub.guests.iter().find(|g| g.uuid == own).map(|g| (g.uuid, g.name.clone())),
        Some(name) => host.hub.guests.iter().find(|g| g.placed && g.name.eq_ignore_ascii_case(name)).map(|g| (g.uuid, g.name.clone())),
    }
}

/// `/tb ...`, run for an operator.
pub(super) fn perform_command(_game: &mut Game<'_>, player: &horizon_client::mod_api::NativeServerPlayer, command: &str, ran: &mut bool) {
    if command.split_whitespace().next() != Some("tb") || player.permission < 2 {
        return;
    }
    *ran = true;
    // Run at the next tick: inside the command the player's state is swapped
    // in, and changes made to their inventory here would be lost.
    PENDING_COMMANDS.with(|p| p.borrow_mut().push((player.uuid, player.position, command.to_owned())));
}

thread_local! {
    /// `/tb` commands to run at the next tick: who, where they stood, the command.
    static PENDING_COMMANDS: RefCell<Vec<(u128, [f64; 3], String)>> = const { RefCell::new(Vec::new()) };
}

/// The `/tb` commands waiting, run in their player's level's pass (a placed
/// backpack is reached there); a player who left drops theirs.
fn run_commands(game: &mut Game<'_>) {
    let here = game.dim.id().to_owned();
    let waiting = PENDING_COMMANDS.with(|p| std::mem::take(&mut *p.borrow_mut()));
    for (own, at, command) in waiting {
        let dimension = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == own)).map(|g| g.dimension.clone());
        match dimension {
            Some(d) if d == here => run_command(game, own, at, &command),
            Some(_) => PENDING_COMMANDS.with(|p| p.borrow_mut().push((own, at, command))),
            None => {}
        }
    }
}

/// One `/tb` command, its answer in the player's chat.
fn run_command(game: &mut Game<'_>, own: u128, position: [f64; 3], command: &str) {
    let words: Vec<&str> = command.split_whitespace().collect();
    let say = |game: &mut Game<'_>, text: String, ok: bool| {
        game.native_tell(own, Text::literal(if ok { text } else { format!("§c{text}") }));
    };
    let pos = |w: &[&str]| -> Option<[i32; 3]> {
        Some([command_coord(w.first()?, position[0])?, command_coord(w.get(1)?, position[1])?, command_coord(w.get(2)?, position[2])?])
    };
    let rest = &words[1..];
    match rest {
        ["access", args @ ..] if args.len() == 3 => {
            let Some(at) = pos(args) else { return };
            if tb::is_backpack(&game.native_block(at)) && game.native_mod_block_entity(at).is_some() {
                with(|s| s.access.insert(own));

                open_later_for(game, own, Source::Block(at));
                say(game, format!("Accessing backpack of {}, {}, {}", at[0], at[1], at[2]), true);
            } else {
                say(game, format!("There's no backpack at coordinates {}, {}, {}", at[0], at[1], at[2]), false);
            }
        }
        ["access", target] => match command_player(game, Some(target), own) {
            Some((uuid, name)) if worn(game, uuid).is_some() => {
                open_later_for(game, own, if uuid == own { Source::Worn } else { Source::Other(uuid) });
                say(game, format!("Accessing backpack of {name}"), true);
            }
            _ => say(game, "Can't access backpack".into(), false),
        },
        ["remove" | "clear", args @ ..] => {
            let clear = rest[0] == "clear";
            let Some((uuid, name)) = command_player(game, args.first().copied(), own) else {
                say(game, "No player was found".into(), false);
                return;
            };
            let Some(stack) = worn(game, uuid) else {
                say(game, format!("Player {name} is not wearing backpack"), false);
                return;
            };
            close_worn_menus(game, uuid);
            // A copy into the player's inventory (or at their feet).
            give_or_drop(game, uuid, stack.clone());
            let kept = clear.then(|| {
                let mut bare = ItemStack::new(&stack.item, 1);
                set_item_tier(game, &mut bare, stack_tier(&stack));
                bare
            });
            set_worn(game, uuid, kept.as_ref());
            tell_worn(game, uuid);
            let text = if clear {
                format!("Cleared contents of Traveler's Backpack from {name} and added copy to inventory")
            } else {
                format!("Removed Traveler's Backpack from {name} and added copy to inventory")
            };
            say(game, text, true);
        }
        ["restore", target, id] => {
            let Some((uuid, name)) = command_player(game, Some(target), own) else {
                say(game, "No player was found".into(), false);
                return;
            };
            match read_backup(game, id) {
                Some(stack) => {
                    give_or_drop(game, uuid, stack);
                    say(game, format!("Successfully restored {name}'s backpack"), true);
                }
                None => say(game, format!("Backpack with ID {id} not found"), false),
            }
        }
        ["unpack", args @ ..] if args.len() == 3 => {
            let Some(at) = pos(args) else { return };
            if !tb::is_backpack(&game.native_block(at)) {
                say(game, format!("There's no backpack at coordinates {}, {}, {}", at[0], at[1], at[2]), false);
                return;
            }
            flush_block_menus(game, at);
            let tier = game.native_mod_block_entity(at).map(|m| placed_tier(&m.nbt, &m.items));
            let mut drops = Vec::new();
            game.native_with_mod_block_entity(at, |m| {
                let end = tier.map_or(0, tb::Tier::slots).min(m.items.len());
                drops = m.items[..end].iter_mut().filter_map(Option::take).filter(|s| s.count > 0).collect();
            });
            if drops.is_empty() {
                say(game, format!("There's no contents in backpack at coordinates {}, {}, {}", at[0], at[1], at[2]), false);
                return;
            }
            let dimension = game.dim.id().to_owned();
            set_block_tanks(game, &dimension, at, false);
            let spot = at.map(|c| f64::from(c) + 0.5);
            for stack in drops {
                game.spawn_item(stack, spot, super::dispenser::new_item_motion(), 10);
            }
            say(game, format!("Dropping contents of backpack placed at {}, {}, {}", at[0], at[1], at[2]), true);
        }
        ["unpack", target] => {
            let Some((uuid, name)) = command_player(game, Some(target), own) else {
                say(game, "No player was found".into(), false);
                return;
            };
            let Some(mut stack) = worn(game, uuid) else {
                say(game, format!("Player {name} is not wearing backpack"), false);
                return;
            };
            close_worn_menus(game, uuid);
            let tier = stack_tier(&stack);
            let items = contents(&stack, tier.slots());
            let drops: Vec<ItemStack> = items.into_iter().flatten().filter(|s| s.count > 0).collect();
            if drops.is_empty() {
                say(game, format!("There's no contents in {name} backpack"), false);
                return;
            }
            set_contents(&mut stack, vec![None; tier.slots()]);
            set_worn(game, uuid, Some(&stack));
            tell_worn(game, uuid);
            let at = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.position).unwrap_or(position);
            let feet = at.map(|c| c.floor());
            for stack in drops {
                game.spawn_item(stack, [feet[0] + 0.5, feet[1] + 0.5, feet[2] + 0.5], super::dispenser::new_item_motion(), 10);
            }
            say(game, format!("Dropping contents of {name} backpack at {}, {}, {}", feet[0], feet[1], feet[2]), true);
        }
        _ => say(game, "Unknown or incomplete command".into(), false),
    }
}

/// A menu open for `viewer` at the next tick, as a click would open it.
fn open_later_for(_game: &mut Game<'_>, viewer: u128, source: Source) {
    let dimension = _game.dim.id().to_owned();
    with(|s| {
        if !s.opening.iter().any(|(u, _, _)| *u == viewer) {
            s.opening.push((viewer, source, dimension));
        }
    });
}

/// The menus showing `uuid`'s worn backpack close first (their contents saved).
fn close_worn_menus(game: &mut Game<'_>, uuid: u128) {
    let viewers: Vec<(u128, i32)> = with(|s| {
        s.open.iter().filter(|(v, o)| (**v == uuid && o.0 == Source::Worn) || o.0 == Source::Other(uuid)).map(|(v, o)| (*v, o.2)).collect()
    });
    for (viewer, window) in viewers {
        let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == viewer)).map(|g| g.conn) else {
            continue;
        };
        if let Some(host) = game.lan.as_mut() {
            horizon_client::lan::containers::travelersbackpack_menu::close(&mut host.hub, conn, window);
        }
    }
}

/// `BackpackManager`'s folder: `<world>/backpacks/<player uuid>/`.
fn backups_dir(game: &Game<'_>) -> Option<std::path::PathBuf> {
    Some(game.native_world_dir()?.join("backpacks"))
}

fn uuid_text(uuid: u128) -> String {
    let h = format!("{uuid:032x}");
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

/// `BackpackManager.addBackpack`: a dead player's backpack kept in a file
/// named by its item and the time (`travelersbackpack.cow_12.11.2026-21.05.33.dat`).
fn save_backup(game: &Game<'_>, uuid: u128, stack: &ItemStack) {
    let Some(dir) = backups_dir(game) else {
        return;
    };
    let dir = dir.join(uuid_text(uuid));
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
    // Days since 1970 to a date (Howard Hinnant's `civil_from_days`).
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let name = format!(
        "{}_{day:02}.{month:02}.{year:04}-{:02}.{:02}.{:02}.dat",
        stack.item.replace(':', "."),
        rest / 3600,
        rest / 60 % 60,
        rest % 60
    );
    let root = horizon_world::nbt::NamedTag { name: "".into(), tag: horizon_items::item_nbt::item_to_nbt(stack) };
    if let Ok(bytes) = horizon_world::nbt::write_named(&root, horizon_world::nbt::Limits::default()) {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(name), bytes);
    }
}

/// `BackpackManager.getBackpack`: a backup by its file name, whoever's it is.
fn read_backup(game: &Game<'_>, id: &str) -> Option<ItemStack> {
    if id.contains(['/', '\\']) || id.contains("..") {
        return None;
    }
    let dir = backups_dir(game)?;
    for player in std::fs::read_dir(&dir).ok()?.flatten() {
        // The file's name, with or without its `.dat`.
        let path = player.path().join(id);
        let bytes = std::fs::read(&path).or_else(|_| std::fs::read(player.path().join(format!("{id}.dat"))));
        if let Ok(bytes) = bytes {
            let (root, _) = horizon_world::nbt::read_named(&bytes, horizon_world::nbt::Limits::default()).ok()?;
            return horizon_items::item_nbt::item_from_nbt(&root.tag);
        }
    }
    None
}

/// Whether `uuid`'s worn backpack is open: by them, or by an operator (`/tb access`).
fn worn_menu_open(uuid: u128) -> bool {
    with(|s| s.open.iter().any(|(v, o)| (*v == uuid && o.0 == Source::Worn) || o.0 == Source::Other(uuid)))
}

thread_local! {
    /// The game time of the last tick's first level pass.
    static LAST_TICK: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
}

/// Each player's waiting actions, at most 64: a flood of button presses (or
/// actions for a level that doesn't tick) can't grow the queue for ever.
fn cap_actions() {
    with(|s| {
        let mut count: HashMap<u128, usize> = HashMap::new();
        s.actions.retain(|(u, _, _)| {
            let n = count.entry(*u).or_insert(0);
            *n += 1;
            *n <= 64
        });
    });
}

/// Menus of placed backpacks closed while their level wasn't ticking: written
/// into them when it does.
fn write_closed_elsewhere(game: &mut Game<'_>) {
    let here = game.dim.id().to_owned();
    let due: Vec<([i32; 3], Vec<Option<ItemStack>>)> = with(|s| {
        let (due, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.closed_elsewhere).into_iter().partition(|(_, d, _)| *d == here);
        s.closed_elsewhere = rest;
        due.into_iter().map(|(p, _, i)| (p, i)).collect()
    });
    for (pos, items) in due {
        if !tb::is_backpack(&game.native_block(pos)) {
            continue;
        }
        let tier = game.native_mod_block_entity(pos).map(|m| placed_tier(&m.nbt, &m.items));
        if let Some(tier) = tier {
            set_block_tanks(game, &here, pos, shows_tanks(&items, tier));
        }
        game.native_with_mod_block_entity(pos, |m| {
            if m.id != tb::BLOCK_ENTITY {
                return;
            }
            for (i, item) in items.into_iter().enumerate() {
                if let Some(slot) = m.items.get_mut(i) {
                    *slot = item;
                }
            }
        });
    }
}

/// The acting player has a backpack open from the selected slot.
pub(super) fn held_locked(game: &mut Game<'_>) -> bool {
    let uuid = game.native_acting_id();
    with(|s| s.open.get(&uuid).is_some_and(|o| o.0 == Source::Hand(game.selected)))
}

/// `player` has a backpack open from their inventory.
pub(super) fn hand_menu_open(player: u128) -> bool {
    with(|s| s.open.get(&player).is_some_and(|o| matches!(o.0, Source::Hand(_))))
}

/// `RightClickHandler`'s quick unequip, with `rightClickUnequip` (off by
/// default): sneaking, the main hand empty, a right click on a block (not a
/// backpack) places the worn backpack against its face, facing the player.
pub(super) fn quick_unequip(game: &mut Game<'_>, pos: [i32; 3], face: [i32; 3], main: bool, cancel: &mut bool) {
    if *cancel || !cfg().right_click_unequip || !main || !game.native_sneaking() {
        return;
    }
    let uuid = game.native_acting_id();
    let Some(g) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)) else {
        return;
    };
    if g.inventory.get(g.selected).is_some_and(Option::is_some) || tb::is_backpack(&game.native_block(pos)) {
        return;
    }
    let yaw = g.yaw;
    let Some(stack) = worn(game, uuid) else {
        return;
    };
    if worn_menu_open(uuid) {
        return;
    }
    let spot = [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]];
    if !replaceable(&game.native_block(spot)) {
        return;
    }
    let dimension = game.dim.id().to_owned();
    let Some(state) = game.native_default_state(&stack.item) else {
        return;
    };
    if !game.native_set_block(&dimension, spot, state, 3) {
        return;
    }
    // Its front toward the player (`getHorizontalDirection().getOpposite()`).
    let facing = match facing_of_yaw(yaw) {
        "north" => "south",
        "south" => "north",
        "east" => "west",
        _ => "east",
    };
    if let Some(turned) = with_property(game, spot, "facing", facing) {
        game.native_set_block(&dimension, spot, turned, 3);
    }
    place_block(game, &stack, spot);
    set_worn(game, uuid, None);
    tell_worn(game, uuid);
    *cancel = true;
}

// ---- mobs with backpacks (`MobEntityMixin`, `HumoanoidMobRenderer`, `DeathHandler`) ----

/// `MobEntityMixin.initialize`: a zombie, skeleton or enderman (the nether's:
/// a wither skeleton or piglin) that spawns, grown and with nothing on its
/// chest, wears a random backpack now and then (`spawnEntitiesWithBackpack`,
/// `chance`): a random bag; half of them with the tanks upgrade; most dyed.
pub(super) fn mob_spawned(game: &mut Game<'_>, dimension: &str, kind: &str, pos: [f64; 3]) {
    use horizon_core::random::RandomSource as _;
    let c = cfg();
    if !c.spawn_entities_with_backpack || game.dim.id() != dimension {
        return;
    }
    let nether = c.nether_entity_types.iter().any(|k| k == kind);
    if !nether && !c.overworld_entity_types.iter().any(|k| k == kind) {
        return;
    }
    let Some(i) = game.mobs.iter().rposition(|m| {
        !m.removed && *m.kind.id == *kind && m.position.x == pos[0] && m.position.y == pos[1] && m.position.z == pos[2]
    }) else {
        return;
    };
    let mob = &game.mobs[i];
    if mob.is_baby() || mob.extra.armor[1].is_some() || mob.extra.chest.is_some() {
        return;
    }
    let seed = i64::from(mob.id) ^ game.world.game_time.wrapping_mul(0x5DEE_CE66D);
    let mut random = horizon_core::random::LegacyRandom::new(seed);
    if f64::from(random.next_float()) >= c.spawn_chance {
        return;
    }
    let list = if nether { &c.nether_backpacks } else { &c.overworld_backpacks };
    if list.is_empty() {
        return;
    }
    let pick = random.next_int_between_inclusive(0, list.len() as i32 - 1).unwrap_or(0) as usize;
    let mut backpack = ItemStack::new(&list[pick], 1);
    if !tb::is_backpack(&backpack.item) {
        return;
    }
    set_item_bag(&mut backpack, random.next_int_between_inclusive(0, 15).unwrap_or(14));
    if random.next_float() > 0.5 {
        let tier = configured_tier(0);
        let mut items = vec![None; tier.slots()];
        items[tier.storage] = Some(ItemStack::new(TANKS_UPGRADE, 1));
        set_contents(&mut backpack, items);
    }
    if random.next_float() > 0.25 {
        set_item_dye(&mut backpack, Some(random.next_int() & 0xFF_FFFF));
    }
    refresh_look(&mut backpack);
    let mob = &mut game.mobs[i];
    mob.extra.chest = Some(std::sync::Arc::from(&*backpack.item));
    mob.extra.armor[1] = Some(backpack);
}

/// `DeathHandler`: a mob wearing a backpack that a player killed drops it
/// (unless its equipment's own drop already did).
fn mob_backpack_drop(game: &Game<'_>, context: &NativeLootDrops<'_>, drops: &mut Vec<ItemStack>) {
    let (Some((id, _)), Some(_)) = (context.entity, context.player) else {
        return;
    };
    if drops.iter().any(|d| tb::is_backpack(&d.item)) {
        return;
    }
    let worn = game.mobs.iter().find(|m| m.id == id).and_then(|m| m.extra.armor[1].clone()).filter(|s| tb::is_backpack(&s.item));
    if let Some(stack) = worn {
        drops.push(stack);
    }
}

/// `HumoanoidMobRenderer`: a mob's backpack on its back, as a player's.
fn mob_backpack_layer(
    game: &Game<'_>,
    mob: &super::native_mob_draw::NativeMobRender<'_>,
    draw: &mut super::native_block_entity_draw::BlockEntityDraw,
) {
    let Some(m) = game.mobs.iter().find(|m| m.id == mob.id) else {
        return;
    };
    let Some(stack) = m.extra.armor[1].clone().filter(|s| tb::is_backpack(&s.item)) else {
        return;
    };
    let tier = stack_tier(&stack);
    let tanks = shows_tanks(&contents(&stack, tier.slots()), tier);
    let bag = bag_name(item_bag(&stack));
    // The body's top a little higher on a taller mob (an enderman).
    let lift = (f64::from(m.height()) - 1.95).max(0.0) as f32;
    draw.push();
    draw.translate([-0.5, 0.82 + lift, -0.125 - 11.8 / 16.0]);
    let tanks = if tanks { "true" } else { "false" };
    let dye = item_dye(&stack);
    if &*stack.item == STANDARD {
        let dyed = if dye.is_some() { "true" } else { "false" };
        game.native_draw_block(draw, &stack.item, &[("bag", bag), ("dyed", dyed), ("facing", "south"), ("tanks", tanks)]);
        if let Some(colour) = dye {
            draw_dyed_body(game, draw, colour, model_turn("south"));
        }
    } else {
        game.native_draw_block(draw, &stack.item, &[("bag", bag), ("facing", "south"), ("tanks", tanks)]);
    }
    draw.pop();
}

/// Playing jukebox upgrades among `stacks` stop (`JukeboxUpgrade.onUpgradeRemoved`).
fn stop_jukeboxes<'a>(game: &mut Game<'_>, stacks: impl Iterator<Item = &'a mut ItemStack>) {
    let mut sounds = Vec::new();
    for stack in stacks {
        if tb::Tab::of(Some(&stack.item)) == tb::Tab::Jukebox
            && let Some(sound) = playing(stack)
        {
            set_playing(stack, None);
            sounds.push(sound);
        }
    }
    if let Some(host) = game.lan.as_mut() {
        for sound in sounds {
            horizon_client::lan::containers::travelersbackpack_menu::stop_sound(&mut host.hub, &sound);
        }
    }
}

/// A jukebox upgrade still marked playing in a player's inventory or on their
/// cursor was taken out of its backpack: its song stops.
fn loose_jukeboxes(game: &mut Game<'_>) {
    let mut sounds = Vec::new();
    if let Some(host) = game.lan.as_mut() {
        for g in host.hub.guests.iter_mut() {
            for stack in g.inventory.iter_mut().chain(std::iter::once(&mut g.carried)).chain(std::iter::once(&mut g.offhand)).flatten() {
                if tb::Tab::of(Some(&stack.item)) == tb::Tab::Jukebox
                    && let Some(sound) = playing(stack)
                {
                    set_playing(stack, None);
                    sounds.push(sound);
                }
            }
        }
        for sound in sounds {
            horizon_client::lan::containers::travelersbackpack_menu::stop_sound(&mut host.hub, &sound);
        }
    }
}

/// A jukebox upgrade played in a carried or worn backpack: its player and song.
const JUKEBOX_CHANNEL: &str = "travelersbackpack:jukebox";

thread_local! {
    /// Songs heard from the server, to start at this client's next tick.
    static SONGS: RefCell<Vec<(i32, String)>> = const { RefCell::new(Vec::new()) };
}

/// `JukeboxWidget.playSound`: the song follows the player carrying it
/// (`MovingSound`, an entity-bound sound of the records).
pub(super) fn play_songs(game: &mut Game<'_>) {
    for (entity, sound) in SONGS.with(|s| std::mem::take(&mut *s.borrow_mut())) {
        let id = entity as u32;
        let Some(state) = game.sound_entity_state(id) else {
            continue;
        };
        let bound = horizon_client::sound_engine::SoundEntity::new(id, state.position, state.silent);
        game.audio.play_bound(&sound, horizon_client::sound_engine::RECORDS, 4.0, 1.0, bound, None);
    }
}

/// `Level.playSound(null, …)`: every player in `uuid`'s level hears it,
/// fainter the farther they are, not only `uuid`.
fn play_near(game: &mut Game<'_>, uuid: u128, sound: &str, category: &str, at: Option<[f64; 3]>, volume: f32, pitch: f32) -> bool {
    let players: Vec<u128> = game
        .lan
        .as_ref()
        .and_then(|h| {
            let dimension = &h.hub.guests.iter().find(|g| g.uuid == uuid)?.dimension;
            Some(h.hub.guests.iter().filter(|g| g.placed && g.dimension == *dimension).map(|g| g.uuid).collect())
        })
        .unwrap_or_else(|| vec![uuid]);
    let mut played = false;
    for player in players {
        played |= game.native_play_sound_to(player, sound, category, at, volume, pitch);
    }
    played
}
