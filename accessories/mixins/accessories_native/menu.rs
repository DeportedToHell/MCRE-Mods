// The accessories screen's menu on the server (`AccessoriesMenu`): its slots
// are the player's accessories and their cosmetics, held by the server's
// native menu (`Slots::Native`), taking and giving items by vanilla's click
// rules (the whole menu is its input store); the player's inventory follows
// them. Each slot keeps what it may hold (`AccessoriesBasedSlot.mayPlace`,
// the slot type's validators), asked by the menu's own rules
// (`View::may_place`, `slot_max`, `may_pickup`, `quick_move`). Spliced into
// the hub's container code (`lan/containers.rs`).
// Ported from Accessories by Wisp Forest (MIT).
use super::*;

/// What a menu slot is: its slot type, whether it is the cosmetic one, the
/// slot type's validators in order, and the content options that apply
/// (`allowBannerEquip`/`validBannerSlots` and the glider's and totem's).
/// The armour slots in the menu's order (`EquipmentSlot`), head first.
pub const ARMOR: [&str; 4] = ["head", "chest", "legs", "feet"];

/// What a menu slot holds: an accessory or cosmetic (its slot type's), a
/// worn armour piece (`guest.armor`, head first) or the off hand.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Accessory,
    Armor(usize),
    Offhand,
}

#[derive(Clone, Debug, Default)]
pub struct SlotRule {
    pub kind: Kind,
    pub slot: String,
    pub cosmetic: bool,
    pub validators: Vec<String>,
    pub banner: bool,
    pub glider: bool,
    pub totem: bool,
    /// The item tags the content options name (`valid_glider_equip`,
    /// `valid_totem_equip`) are asked for the cape and charm only.
    pub unique: bool,
}

/// Opens menu type `menu_kind` for guest `conn` over their inventory,
/// holding `items` (one a rule), titled `title`; `layout` goes ahead as the
/// menu's extra data (`horizon:menu_extra`), for its screen. The window's id,
/// or None for an unknown menu type or guest.
pub fn open(
    hub: &mut Hub,
    conn: u32,
    menu_kind: &str,
    title: Tag,
    items: Vec<Option<ItemStack>>,
    layout: &[u8],
    rules: Vec<SlotRule>,
) -> Option<i32> {
    let kind = hub.sync.builtin.get("minecraft:menu").and_then(|m| m.id(menu_kind))?;
    let i = hub.guests.iter().position(|g| g.conn == conn)?;
    hub.close_container_at(i);
    let size = items.len();
    let g = &mut hub.guests[i];
    g.native_menu = items.clone();
    g.next_window = g.next_window % 100 + 1;
    let window = g.next_window;
    let message = horizon_protocol::menu_extra::Message { window, pos: [0; 3], data: layout.to_vec() };
    if let Ok(data) = horizon_protocol::menu_extra::encode(&message) {
        g.send(&C::CustomPayload(CustomPayload { channel: horizon_protocol::menu_extra::CHANNEL.into(), data }));
    }
    g.send(&C::OpenScreen(play::OpenScreen { container: window, menu: kind, title }));
    g.menu = Some(Menu {
        window,
        kind,
        pos: None,
        slots: Slots::Native,
        size,
        state: 0,
        sent: Vec::new(),
        drag: None,
        sent_data: Vec::new(),
        hidden: Vec::new(),
        mod_type: None,
    });
    hub.send_menu_content(i, items);
    hub.set_native_store(conn, NativeStore::Input(0..size));
    if let Ok(mut open) = OPEN.lock() {
        open.retain(|o| o.0 != conn);
        open.push((conn, window, rules));
    }
    Some(window)
}

/// Closes `conn`'s menu `window` from the server (`ServerPlayer.closeContainer`):
/// its slots go to `native_menu_input_closed`. False when that isn't the menu open.
pub fn close(hub: &mut Hub, conn: u32, window: i32) -> bool {
    let Some(i) = hub.guests.iter().position(|g| g.conn == conn) else {
        return false;
    };
    if hub.guests[i].menu.as_ref().is_none_or(|m| m.window != window) {
        return false;
    }
    hub.close_container_at(i);
    true
}

/// What guest `conn`'s open menu `window` holds now, if it is still open.
pub fn items(hub: &Hub, conn: u32, window: i32) -> Option<Vec<Option<ItemStack>>> {
    let g = hub.guests.iter().find(|g| g.conn == conn)?;
    g.menu.as_ref().filter(|m| m.window == window)?;
    Some(g.native_menu.clone())
}

/// Guest `conn`'s open menu `window` holds `items` from now on, sent to them at once.
pub fn set_items(hub: &mut Hub, conn: u32, window: i32, items: Vec<Option<ItemStack>>) {
    let Some(i) = hub.guests.iter().position(|g| g.conn == conn) else {
        return;
    };
    if hub.guests[i].menu.as_ref().is_none_or(|m| m.window != window) {
        return;
    }
    hub.guests[i].native_menu = items.clone();
    hub.send_menu_content(i, items);
}

/// Sends guest `conn`'s menu `window` its data slots (`ContainerSetData`, 16
/// bits each on the wire): which of its slots hold something, for its screen.
pub fn send_data(hub: &mut Hub, conn: u32, window: i32, data: &[i32]) {
    let Some(g) = hub.guests.iter_mut().find(|g| g.conn == conn) else {
        return;
    };
    for (k, &value) in data.iter().enumerate() {
        g.send(&C::ContainerSetData(play::ContainerSetData { container: window, id: k as i16, value: value as i16 }));
    }
}

/// The accessories menus open: a guest, its window and each slot's rule.
static OPEN: std::sync::Mutex<Vec<(u32, i32, Vec<SlotRule>)>> = std::sync::Mutex::new(Vec::new());

/// Guest `conn`'s accessories menu is gone.
pub fn forget(conn: u32) {
    if let Ok(mut open) = OPEN.lock() {
        open.retain(|o| o.0 != conn);
    }
}

/// The rule of slot `i` of guest `conn`'s window `window`, when that is an
/// accessories menu and `i` one of its own slots.
pub fn rule(conn: u32, window: i32, i: usize) -> Option<SlotRule> {
    let open = OPEN.lock().ok()?;
    let (_, _, rules) = open.iter().find(|o| o.0 == conn && o.1 == window)?;
    rules.get(i).cloned()
}

/// How many of guest `conn`'s window `window`'s slots are the accessories',
/// when it is an accessories menu.
pub fn own_slots(conn: u32, window: i32) -> Option<usize> {
    let open = OPEN.lock().ok()?;
    open.iter().find(|o| o.0 == conn && o.1 == window).map(|o| o.2.len())
}

/// A validator's answer (`ActionResponse`): valid, invalid, or no say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    Valid,
    Invalid,
    Irrelevant,
}

/// The item tag a slot's `accessories:tag` validator asks
/// (`AccessoriesTags.getTagForSlot`): `accessories:<slot>`, or the unique
/// slot's own id.
pub fn slot_tag(slot: &str) -> String {
    if slot.contains(':') { slot.to_owned() } else { format!("accessories:{slot}") }
}

/// `SlotValidatorRegistry`: what validator `id` says of `stack` in the rule's slot.
fn answer(id: &str, rule: &SlotRule, stack: &ItemStack, items: &crate::items::Items) -> Answer {
    let item = &*stack.item;
    match id {
        // `isValidSlot` by item tags: the slot's own, or `accessories:any`.
        "accessories:tag" => {
            if items.item_in_tag(&slot_tag(&rule.slot), item) || items.item_in_tag("accessories:any", item) {
                Answer::Valid
            } else {
                Answer::Irrelevant
            }
        }
        // As shipped, `ALL` answers false (`ActionResponse.of(false, ..)`).
        "accessories:all" | "accessories:none" => Answer::Invalid,
        // `VanillaItemPredicates.ELYTRA_PREDICATE`.
        "accessories:elytra_item" => {
            if !items.get(item).is_some_and(|i| i.glider) {
                Answer::Irrelevant
            } else if rule.glider || (rule.slot == "cape" && items.item_in_tag("accessories:valid_glider_equip", item)) {
                Answer::Valid
            } else {
                Answer::Irrelevant
            }
        }
        // `TOTEM_PREDICATE`, as shipped: it answers only for stacks *without*
        // death protection.
        "accessories:totem_item" => {
            if item == "minecraft:totem_of_undying" {
                Answer::Irrelevant
            } else if rule.totem || (rule.slot == "charm" && items.item_in_tag("accessories:valid_totem_equip", item)) {
                Answer::Valid
            } else {
                Answer::Irrelevant
            }
        }
        // `BANNER_PREDICATE`: a `BannerItem` where the config allows it.
        "accessories:banner_item" => {
            if item.ends_with("_banner") && item.starts_with("minecraft:") && rule.banner {
                Answer::Valid
            } else {
                Answer::Irrelevant
            }
        }
        // `ArmorSlotTypes`: the cosmetic armour slots take what is worn there.
        v if v.starts_with("accessories_cosmetics:") => {
            let want = &v["accessories_cosmetics:".len()..];
            let slot = items.get(item).and_then(|i| i.equip_slot.as_deref());
            let worn = match want {
                "animal_body" => slot == Some("body"),
                w => slot == Some(w),
            };
            if worn { Answer::Valid } else { Answer::Irrelevant }
        }
        // The component and attribute validators read Accessories' own item
        // components, which no item has here.
        _ => Answer::Irrelevant,
    }
}

/// `AccessoriesAPI.canInsertIntoSlot`: the first validator with a say decides;
/// none: not valid.
pub fn valid(rule: &SlotRule, stack: &ItemStack, items: &crate::items::Items) -> bool {
    match rule.kind {
        Kind::Armor(k) => return items.get(&stack.item).and_then(|i| i.equip_slot.as_deref()) == Some(ARMOR[k]),
        Kind::Offhand => return true,
        Kind::Accessory => {}
    }
    for id in &rule.validators {
        match answer(id, rule, stack, items) {
            Answer::Valid => return true,
            Answer::Invalid => return false,
            Answer::Irrelevant => {}
        }
    }
    false
}
