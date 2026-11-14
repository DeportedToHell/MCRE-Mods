// Accessories in the game. The server reads the slot types, groups and
// entity bindings from the data (`SlotTypeLoader`, `SlotGroupLoader`,
// `EntitySlotLoader`), keeps each player's accessories and cosmetics in their
// saved data (the `accessories:inventory_holder` attachment), opens the
// accessories menu (`AccessoriesMenu`) from the inventory's button or the
// key, equips an accessory used from the hand, drops accessories on death by
// their slot's rule and tells every player what each wears, for its model.
// The client draws the accessories screen (`AccessoriesScreen`) and each
// player's accessories on their model (`AccessoriesRenderLayer` with
// `DefaultAccessoryRenderer`).
//
// Uses are heard while the player's state is swapped in, the hub out of the
// game's hands: what they ask is done at the next server tick.
// Ported from Accessories by Wisp Forest (MIT).
use super::Game;
use horizon_client::items::ItemStack;
use horizon_client::lan::containers::accessories_menu as menu;
use horizon_client::mod_api::NativeScreen;
use horizon_client::text::Text;
use horizon_world::nbt::{Compound, JavaString, Tag};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, HashSet};

/// The native mod's id (`horizon-native.json`): its key's line and its folder.
const MOD_ID: &str = "accessories_native";
/// The port's version.
const VERSION: &str = "0.2.0";
/// The content mod's menu type for the accessories screen.
const MENU: &str = "accessories:accessories_menu";
/// Where a player's accessories are kept in their saved data
/// (`AccessoriesHolderImpl`'s attachment).
const HOLDER: &str = "accessories:inventory_holder";
/// A player's screen options (`PlayerOptions`).
const OPTIONS: &str = "accessories:player_options";

/// The client asks for its accessories screen (`ScreenOpen`).
const OPEN_CHANNEL: &str = "accessories:screen_open";
/// A slot's render toggle (`SyncCosmeticToggle`): the menu slot, two bytes.
const TOGGLE_CHANNEL: &str = "accessories:toggle";
/// A screen option changed (`SyncOptionChange`): `name=value`.
const OPTION_CHANNEL: &str = "accessories:option";
/// What a player wears, for their model (`SyncEntireContainer`).
const WORN_CHANNEL: &str = "accessories:worn";
/// The player's slots, for the clients' item tooltips.
const SLOTS_CHANNEL: &str = "accessories:slots";

fn s(js: &JavaString) -> String {
    String::from_utf16_lossy(&js.0)
}

fn jkey(text: &str) -> JavaString {
    JavaString::from(text)
}

// ---- the data: slot types, groups, bindings ---------------------------------------------

/// `DropRule`: what happens to a slot's stacks when its wearer dies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DropRule {
    Keep,
    Drop,
    Destroy,
    #[default]
    Default,
}

impl DropRule {
    fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "keep" => Some(Self::Keep),
            "drop" => Some(Self::Drop),
            "destroy" => Some(Self::Destroy),
            "default" => Some(Self::Default),
            _ => None,
        }
    }
}

/// `SlotType`: a kind of accessory slot.
#[derive(Clone, Debug)]
struct SlotType {
    name: String,
    amount: usize,
    order: i32,
    /// Its empty slot's sprite (`accessories:container/slot/ring`).
    icon: String,
    validators: Vec<String>,
    drop_rule: DropRule,
    /// A unique slot (`ns:path`), registered by code; its group is hidden.
    unique: bool,
}

/// `SlotGroup`: slots shown together, with their icon.
#[derive(Clone, Debug)]
struct Group {
    name: String,
    order: i32,
    icon: String,
    /// Its slots, in the order they are shown.
    slots: Vec<String>,
    unique: bool,
}

/// Everything the data said, ready to use.
#[derive(Clone, Debug, Default)]
struct Data {
    slots: BTreeMap<String, SlotType>,
    /// In the order they are shown (`SlotGroupLoader`: descending).
    groups: Vec<Group>,
    /// The slots a player has, in `EntitySlotLoader`'s map order: the order
    /// an accessory used from the hand looks for a slot in.
    player: Vec<String>,
}

impl Data {
    fn slot(&self, name: &str) -> Option<&SlotType> {
        self.slots.get(name)
    }
    /// The group a slot is shown in.
    fn group_of(&self, slot: &str) -> Option<&Group> {
        self.groups.iter().find(|g| g.slots.iter().any(|s| s == slot))
    }
    /// The player's slots as the screen shows them: by group, then the
    /// group's order (unique groups are not shown in the grid).
    fn shown_slots(&self) -> Vec<&SlotType> {
        let mut out = Vec::new();
        for g in self.groups.iter().filter(|g| !g.unique) {
            for name in &g.slots {
                if self.player.contains(name)
                    && let Some(t) = self.slots.get(name)
                {
                    out.push(t);
                }
            }
        }
        out
    }
}

/// The JSON files each loader got at the last data load.
#[derive(Default)]
struct Raw {
    entity_tags: HashMap<String, Vec<String>>,
    slots: Vec<(String, serde_json::Value)>,
    groups: Vec<(String, serde_json::Value)>,
}

thread_local! {
    static RAW: RefCell<Raw> = RefCell::new(Raw::default());
    static DATA: RefCell<Option<Data>> = const { RefCell::new(None) };
}

/// The data loaded (the mod's own defaults until a world loaded some).
fn with_data<R>(f: impl FnOnce(&Data) -> R) -> R {
    DATA.with(|d| {
        if d.borrow().is_none() {
            *d.borrow_mut() = Some(default_data());
        }
        f(d.borrow().as_ref().expect("set above"))
    })
}

/// The unique slots Accessories registers itself (`ArmorSlotTypes`): the
/// cosmetic armour, which shows instead of the armour worn.
const COSMETIC_ARMOR: [&str; 6] = ["head", "chest", "legs", "feet", "animal_body", "saddle"];

/// The shipped data, for when no data load reached the mod.
fn default_data() -> Data {
    let slot = |name: &str, amount: usize, order: i32, extra: &[&str]| {
        let mut validators = vec!["accessories:tag".to_owned(), "accessories:component".to_owned()];
        validators.extend(extra.iter().map(|v| (*v).to_owned()));
        (name.to_owned(), amount, order, validators)
    };
    let slots = [
        slot("hat", 1, 1000, &["accessories:banner_item", "accessories:attribute"]),
        slot("face", 1, 900, &["accessories:attribute"]),
        slot("necklace", 1, 1000, &["accessories:attribute"]),
        slot("cape", 1, 900, &["accessories:attribute", "accessories:elytra_item", "accessories:banner_item"]),
        slot("back", 1, 800, &["accessories:attribute", "accessories:elytra_item", "accessories:banner_item"]),
        slot("hand", 2, 900, &["accessories:attribute"]),
        slot("ring", 2, 800, &["accessories:attribute"]),
        slot("wrist", 1, 1000, &["accessories:attribute"]),
        slot("belt", 1, 1000, &["accessories:attribute"]),
        slot("anklet", 1, 1000, &["accessories:attribute"]),
        slot("shoes", 1, 900, &["accessories:attribute"]),
        slot("charm", 1, 1000, &["accessories:attribute", "accessories:totem_item"]),
    ];
    let mut raw_slots = Vec::new();
    for (name, amount, order, validators) in slots {
        let json = serde_json::json!({
            "replace": false, "amount": amount, "operation": "set", "order": order,
            "icon": format!("accessories:container/slot/{name}"), "validators": validators,
        });
        raw_slots.push((format!("accessories:{name}"), json));
    }
    let group = |name: &str, order: i32, slots: &[&str]| {
        (
            format!("accessories:{name}"),
            serde_json::json!({ "order": order, "slots": slots, "icon": format!("accessories:container/slot/group/{name}") }),
        )
    };
    let raw_groups = vec![
        group("misc", 120, &["charm"]),
        group("head", 100, &["hat", "face"]),
        group("chest", 80, &["necklace", "cape", "back"]),
        group("arm", 60, &["hand", "ring", "wrist"]),
        group("leg", 40, &["belt"]),
        group("feet", 20, &["shoes", "anklet"]),
        (
            "accessories:unsorted".to_owned(),
            serde_json::json!({ "order": 10, "slots": [], "icon": "accessories:container/slot/group/unknown" }),
        ),
    ];
    let mut player: Vec<String> =
        ["hat", "face", "necklace", "cape", "back", "hand", "ring", "wrist", "belt", "anklet", "shoes", "charm"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
    player.extend(COSMETIC_ARMOR[..4].iter().map(|s| format!("accessories_cosmetics:{s}")));
    build_data(&raw_slots, &raw_groups, player)
}

/// `SlotTypeLoader`: a file's path is its slot (`data/<any>/accessories/slot/ring.json`
/// is `ring`; a sub-folder names a unique slot's namespace); `any` is no slot.
fn slot_name(id: &str) -> Option<String> {
    let path = id.split_once(':').map_or(id, |(_, p)| p);
    let name = match path.split_once('/') {
        Some((ns, rest)) => format!("{ns}:{rest}"),
        None => path.to_owned(),
    };
    (name != "any" && !name.is_empty()).then_some(name)
}

/// The slot types, groups and the player's slots from the data's files.
fn build_data(raw_slots: &[(String, serde_json::Value)], raw_groups: &[(String, serde_json::Value)], bound: Vec<String>) -> Data {
    // `SlotBuilder`s: every file of a slot adds to its one builder.
    #[derive(Default)]
    struct Builder {
        base: Option<i64>,
        offset: i64,
        order: Option<i32>,
        icon: Option<String>,
        validators: Vec<String>,
        drop: Option<DropRule>,
    }
    let mut builders: BTreeMap<String, Builder> = BTreeMap::new();
    for (id, json) in raw_slots {
        let Some(name) = slot_name(id) else {
            continue;
        };
        let b = builders.entry(name).or_default();
        if let Some(amount) = json.get("amount").and_then(serde_json::Value::as_i64) {
            match json.get("operation").and_then(serde_json::Value::as_str).map(str::to_ascii_lowercase).as_deref() {
                Some("add") => b.offset += amount,
                Some("sub") => b.offset -= amount,
                _ => b.base = Some(amount),
            }
        }
        if let Some(order) = json.get("order").and_then(serde_json::Value::as_i64) {
            b.order = Some(order as i32);
        }
        if let Some(icon) = json.get("icon").and_then(serde_json::Value::as_str) {
            b.icon = Some(icon.to_owned());
        }
        for v in json.get("validators").and_then(serde_json::Value::as_array).into_iter().flatten() {
            if let Some(v) = v.as_str()
                && !b.validators.iter().any(|x| x == v)
            {
                b.validators.push(v.to_owned());
            }
        }
        if let Some(rule) = json.get("dropRule").and_then(serde_json::Value::as_str).and_then(DropRule::parse) {
            b.drop = Some(rule);
        }
    }
    let mut slots = BTreeMap::new();
    for (name, b) in builders {
        let validators = if b.validators.is_empty() {
            vec!["accessories:tag".to_owned(), "accessories:component".to_owned()]
        } else {
            b.validators
        };
        // The config's `modifiers` change the amount at the data load.
        let offset: i64 = config().modifiers.iter().filter(|(s, _)| *s == name).map(|(_, a)| *a).sum();
        let amount = (b.base.unwrap_or(1).max(0) + b.offset + offset).max(0) as usize;
        slots.insert(
            name.clone(),
            SlotType {
                unique: name.contains(':'),
                name,
                amount,
                order: b.order.unwrap_or(1000),
                icon: b.icon.unwrap_or_else(|| "accessories:gui/slot/empty".to_owned()),
                validators,
                drop_rule: b.drop.unwrap_or_default(),
            },
        );
    }
    // `ArmorSlotTypes`: the cosmetic armour slots.
    for part in COSMETIC_ARMOR {
        let name = format!("accessories_cosmetics:{part}");
        slots.insert(
            name.clone(),
            SlotType {
                name,
                amount: 1,
                order: 1000,
                icon: String::new(),
                validators: vec![format!("accessories_cosmetics:{part}")],
                drop_rule: DropRule::Default,
                unique: true,
            },
        );
    }
    // `SlotGroupLoader`: a slot in one group only; the rest are unsorted.
    let mut groups: Vec<Group> =
        vec![Group { name: "unsorted".into(), order: 30, icon: "accessories:gui/group/unknown".into(), slots: Vec::new(), unique: false }];
    let mut grouped: HashSet<String> = HashSet::new();
    for (id, json) in raw_groups {
        let path = id.split_once(':').map_or(id.as_str(), |(_, p)| p);
        if path.contains('/') {
            continue;
        }
        let Some(order) = json.get("order").and_then(serde_json::Value::as_i64) else {
            continue;
        };
        let icon = json.get("icon").and_then(serde_json::Value::as_str).unwrap_or("accessories:gui/group/unknown").to_owned();
        let mut members = Vec::new();
        for slot in json.get("slots").and_then(serde_json::Value::as_array).into_iter().flatten().filter_map(serde_json::Value::as_str) {
            if slots.contains_key(slot) && grouped.insert(slot.to_owned()) {
                members.push(slot.to_owned());
            }
        }
        match groups.iter_mut().find(|g| g.name == path) {
            Some(g) => {
                g.order = order as i32;
                g.icon = icon;
                g.slots.extend(members);
            }
            None => groups.push(Group { name: path.to_owned(), order: order as i32, icon, slots: members, unique: false }),
        }
    }
    for t in slots.values() {
        if grouped.contains(&t.name) {
            continue;
        }
        if t.unique {
            let ns = t.name.split_once(':').map_or("", |(n, _)| n).to_owned();
            match groups.iter_mut().find(|g| g.unique && g.name == ns) {
                Some(g) => g.slots.push(t.name.clone()),
                None => groups.push(Group { name: ns, order: 5, icon: String::new(), slots: vec![t.name.clone()], unique: true }),
            }
        } else if let Some(g) = groups.iter_mut().find(|g| g.name == "unsorted") {
            g.slots.push(t.name.clone());
        }
    }
    // Descending by (order, name), the groups and each group's slots.
    groups.sort_by(|a, b| (b.order, &b.name).cmp(&(a.order, &a.name)));
    for g in &mut groups {
        g.slots.sort_by(|a, b| {
            let (oa, ob) = (slots.get(a).map_or(0, |t| t.order), slots.get(b).map_or(0, |t| t.order));
            (ob, b).cmp(&(oa, a))
        });
    }
    let player = hashmap_order(bound.into_iter().filter(|s| slots.contains_key(s)).collect());
    Data { slots, groups, player }
}

/// `String.hashCode`.
fn java_hash(text: &str) -> i32 {
    text.encode_utf16().fold(0i32, |h, c| h.wrapping_mul(31).wrapping_add(i32::from(c)))
}

/// The order a Java `HashMap<String, _>` holding `names` walks them in
/// (`EntitySlotLoader` keeps an entity's slots in one).
fn hashmap_order(names: Vec<String>) -> Vec<String> {
    let mut unique: Vec<String> = Vec::new();
    for n in names {
        if !unique.contains(&n) {
            unique.push(n);
        }
    }
    let mut cap = 16usize;
    while unique.len() as f64 > cap as f64 * 0.75 {
        cap *= 2;
    }
    let mut buckets: Vec<Vec<String>> = vec![Vec::new(); cap];
    for n in unique {
        let h = java_hash(&n);
        let h = (h ^ ((h as u32) >> 16) as i32) as u32 as usize;
        buckets[h & (cap - 1)].push(n);
    }
    buckets.concat()
}

/// The data load's folders (`AddServerReloadListenersEvent`): entity type
/// tags first (the bindings name them), then the slots, groups and bindings.
pub(super) fn add_reload_listeners(listeners: &mut Vec<super::native_reload_listeners::NativeReloadListener>) {
    use super::native_reload_listeners::NativeReloadListener as L;
    listeners.push(L::json("accessories:entity_type_tags", "tags/entity_type"));
    listeners.push(L::json("accessories:slot_loader", "accessories/slot"));
    listeners.push(L::json("accessories:group_loader", "accessories/group"));
    listeners.push(L::json("accessories:entity_slot_loader", "accessories/entity"));
}

/// A loader's files: the bindings, last, build the data.
pub(super) fn reload_listener_apply(
    listener: &super::native_reload_listeners::NativeReloadListener,
    files: &[(String, serde_json::Value)],
) {
    match listener.id.as_str() {
        "accessories:entity_type_tags" => RAW.with(|r| {
            let mut r = r.borrow_mut();
            r.entity_tags.clear();
            for (id, json) in files {
                let values = json
                    .get("values")
                    .and_then(serde_json::Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str().or_else(|| v.get("id").and_then(serde_json::Value::as_str)))
                    .map(str::to_owned)
                    .collect();
                r.entity_tags.insert(id.clone(), values);
            }
        }),
        "accessories:slot_loader" => RAW.with(|r| r.borrow_mut().slots = files.to_vec()),
        "accessories:group_loader" => RAW.with(|r| r.borrow_mut().groups = files.to_vec()),
        "accessories:entity_slot_loader" => {
            let data = RAW.with(|r| {
                let r = r.borrow();
                // Does an entry (`minecraft:player`, `#ns:tag`) name the player?
                fn names_player(tags: &HashMap<String, Vec<String>>, entry: &str, depth: u8) -> bool {
                    match entry.strip_prefix('#') {
                        Some(tag) => {
                            depth < 8 && tags.get(tag).is_some_and(|v| v.iter().any(|e| names_player(tags, e, depth + 1)))
                        }
                        None => entry == "minecraft:player",
                    }
                }
                let mut bound = Vec::new();
                for (_, json) in files {
                    let player = json
                        .get("entities")
                        .and_then(serde_json::Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(serde_json::Value::as_str)
                        .any(|e| names_player(&r.entity_tags, e, 0));
                    if player {
                        bound.extend(json.get("slots").and_then(serde_json::Value::as_array).into_iter().flatten().filter_map(serde_json::Value::as_str).map(str::to_owned));
                    }
                }
                build_data(&r.slots, &r.groups, bound)
            });
            DATA.with(|d| *d.borrow_mut() = Some(data));
            with(|s| s.reloaded = true);
        }
        _ => {}
    }
}

/// The sprite of a slot's or group's icon (`accessories:container/slot/ring`
/// is the GUI sprite `textures/gui/sprites/container/slot/ring.png`).
fn icon_sprite(icon: &str) -> String {
    match icon.split_once(':') {
        Some((ns, path)) => format!("{ns}:gui/sprites/{path}"),
        None => format!("gui/sprites/{icon}"),
    }
}

// ---- a player's accessories (`AccessoriesHolderImpl`, `AccessoriesContainerImpl`) ---------

/// One slot type's container: its render toggles, accessories and cosmetics,
/// as many as its size, and its size's persistent modifiers (`/accessories
/// slot modifier`): id, amount, operation.
#[derive(Clone, Debug, Default, PartialEq)]
struct Container {
    render: Vec<bool>,
    items: Vec<Option<ItemStack>>,
    cosmetics: Vec<Option<ItemStack>>,
    modifiers: Vec<(String, f64, String)>,
}

impl Container {
    fn sized(size: usize) -> Self {
        Self { render: vec![true; size], items: vec![None; size], cosmetics: vec![None; size], modifiers: Vec::new() }
    }
}

type Holder = BTreeMap<String, Container>;

fn stacks_from(tag: Option<&Tag>, size: usize, invalid: &mut Vec<ItemStack>) -> Vec<Option<ItemStack>> {
    let mut out = vec![None; size];
    if let Some(Tag::List(list)) = tag {
        for entry in list {
            let Tag::Compound(c) = entry else {
                continue;
            };
            let slot = match c.get(&jkey("Slot")) {
                Some(Tag::Int(i)) => *i as usize,
                Some(Tag::Byte(b)) => *b as u8 as usize,
                _ => continue,
            };
            let Some(stack) = horizon_items::item_nbt::item_from_nbt(entry).filter(|s| s.count > 0) else {
                continue;
            };
            match out.get_mut(slot) {
                Some(place) if place.is_none() => *place = Some(stack),
                // `AccessoriesContainerImpl.read`: what no longer fits is given back.
                _ => invalid.push(stack),
            }
        }
    }
    out
}

fn stacks_to(stacks: &[Option<ItemStack>]) -> Tag {
    let mut list = Vec::new();
    for (i, s) in stacks.iter().enumerate() {
        let Some(s) = s else {
            continue;
        };
        if let Tag::Compound(mut c) = horizon_items::item_nbt::item_to_nbt(s) {
            c.insert(jkey("Slot"), Tag::Int(i as i32));
            list.push(Tag::Compound(c));
        }
    }
    Tag::List(list)
}

fn modifiers_from(tag: Option<&Tag>) -> Vec<(String, f64, String)> {
    let Some(Tag::List(list)) = tag else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|t| {
            let Tag::Compound(c) = t else {
                return None;
            };
            let id = match c.get(&jkey("id"))? {
                Tag::String(t) => s(t),
                _ => return None,
            };
            let amount = match c.get(&jkey("amount")) {
                Some(Tag::Double(v)) => f64::from_bits(*v),
                Some(Tag::Float(v)) => f64::from(f32::from_bits(*v)),
                Some(Tag::Int(v)) => f64::from(*v),
                _ => 0.0,
            };
            let op = match c.get(&jkey("operation")) {
                Some(Tag::String(t)) => s(t),
                _ => "add_value".into(),
            };
            Some((id, amount, op))
        })
        .collect()
}

fn modifiers_to(modifiers: &[(String, f64, String)]) -> Tag {
    Tag::List(
        modifiers
            .iter()
            .map(|(id, amount, op)| {
                let mut c = Compound::new();
                c.insert(jkey("id"), Tag::String(jkey(id)));
                c.insert(jkey("amount"), Tag::Double(amount.to_bits()));
                c.insert(jkey("operation"), Tag::String(jkey(op)));
                Tag::Compound(c)
            })
            .collect(),
    )
}

/// `AccessoriesContainerImpl.update`: a slot's size from its base and its
/// modifiers: `+` the added values, `+ base *` the multiplied-base ones, then
/// times each multiplied-total one's amount (as shipped, not `1 + amount`),
/// rounded.
fn slot_size(base: usize, modifiers: &[(f64, String)]) -> usize {
    let base = base as f64;
    let mut size = base;
    for (a, op) in modifiers {
        if op == "add_value" {
            size += a;
        }
    }
    let mut multiplied = 0.0;
    for (a, op) in modifiers {
        if op == "add_multiplied_base" {
            multiplied += a;
        }
    }
    size += base * multiplied;
    for (a, op) in modifiers {
        if op == "add_multiplied_total" {
            size *= a;
        }
    }
    size.round().max(0.0) as usize
}

/// Slot `name`'s size modifiers that worn stacks give (`SlotAttribute`,
/// `accessories:<name>` in their `accessories:attributes`).
fn worn_slot_modifiers(items: &horizon_client::items::Items, saved: &Compound, name: &str) -> Vec<(f64, String)> {
    let attribute = format!("accessories:{}", name.replace(':', "-"));
    let mut out = Vec::new();
    let mut ignored = Vec::new();
    for (slot, tag) in saved {
        let slot = s(slot);
        let Tag::Compound(sc) = tag else {
            continue;
        };
        for stack in stacks_from(sc.get(&jkey("items")), 256, &mut ignored).into_iter().flatten() {
            for m in menu::modifiers(&stack, items).0 {
                if (m.attribute == attribute || m.attribute == format!("accessories:{name}")) && (m.slot == slot || m.slot == "any") {
                    out.push((m.amount, m.operation));
                }
            }
        }
    }
    out
}

/// Player `uuid`'s accessories, sized by the data and the slots' modifiers:
/// what no longer fits (a smaller slot, a slot they lost) is in the second
/// list, to give back. None when the player isn't reached.
fn read_holder(game: &Game<'_>, uuid: u128) -> Option<(Holder, Vec<ItemStack>)> {
    let data = game.native_player_persistent_data(uuid);
    let saved: Compound = match data {
        Some(Tag::Compound(c)) => match c.get(&jkey(HOLDER)) {
            Some(Tag::Compound(h)) => match h.get(&jkey("accessories_containers")) {
                Some(Tag::Compound(containers)) => containers.clone(),
                _ => Compound::new(),
            },
            _ => Compound::new(),
        },
        Some(_) => Compound::new(),
        None => return None,
    };
    let items = game.items();
    let mut invalid = Vec::new();
    let mut holder = Holder::new();
    with_data(|d| {
        for name in &d.player {
            let base = d.slot(name).map_or(0, |t| t.amount);
            let persistent = match saved.get(&jkey(name)) {
                Some(Tag::Compound(sc)) => modifiers_from(sc.get(&jkey("persistent_modifiers"))),
                _ => Vec::new(),
            };
            // Unique slots don't resize (`allowResizing`).
            let size = if name.contains(':') {
                base
            } else {
                let mut mods: Vec<(f64, String)> = persistent.iter().map(|(_, a, o)| (*a, o.clone())).collect();
                mods.extend(worn_slot_modifiers(items, &saved, name));
                slot_size(base, &mods)
            };
            let mut c = Container::sized(size);
            c.modifiers = persistent;
            if let Some(Tag::Compound(sc)) = saved.get(&jkey(name)) {
                c.items = stacks_from(sc.get(&jkey("items")), size, &mut invalid);
                c.cosmetics = stacks_from(sc.get(&jkey("cosmetics")), size, &mut invalid);
                if let Some(Tag::List(r)) = sc.get(&jkey("render_options")) {
                    for (k, t) in r.iter().enumerate().take(size) {
                        if let Tag::Byte(b) = t {
                            c.render[k] = *b != 0;
                        }
                    }
                }
            }
            holder.insert(name.clone(), c);
        }
        // `revalidatePlayer`: a slot type no longer bound gives everything back.
        for (name, tag) in &saved {
            let name = s(name);
            if holder.contains_key(&name) {
                continue;
            }
            if let Tag::Compound(sc) = tag {
                let mut all = stacks_from(sc.get(&jkey("items")), 256, &mut invalid);
                all.extend(stacks_from(sc.get(&jkey("cosmetics")), 256, &mut invalid));
                invalid.extend(all.into_iter().flatten());
            }
        }
    });
    Some((holder, invalid))
}

/// Player `uuid`'s accessories are `holder` from now on. False when the player isn't reached.
fn write_holder(game: &mut Game<'_>, uuid: u128, holder: &Holder) -> bool {
    let mut data = match game.native_player_persistent_data(uuid) {
        Some(Tag::Compound(c)) => c,
        Some(_) => Compound::new(),
        None => return false,
    };
    let mut containers = Compound::new();
    for (name, c) in holder {
        let mut sc = Compound::new();
        sc.insert(jkey("slot_name"), Tag::String(jkey(name)));
        sc.insert(jkey("base_size"), Tag::Int(with_data(|d| d.slot(name).map_or(0, |t| t.amount)) as i32));
        sc.insert(jkey("current_size"), Tag::Int(c.items.len() as i32));
        sc.insert(jkey("render_options"), Tag::List(c.render.iter().map(|&b| Tag::Byte(i8::from(b))).collect()));
        sc.insert(jkey("items"), stacks_to(&c.items));
        sc.insert(jkey("cosmetics"), stacks_to(&c.cosmetics));
        sc.insert(jkey("persistent_modifiers"), modifiers_to(&c.modifiers));
        containers.insert(jkey(name), Tag::Compound(sc));
    }
    let mut h = Compound::new();
    h.insert(jkey("accessories_containers"), Tag::Compound(containers));
    data.insert(jkey(HOLDER), Tag::Compound(h));
    game.native_set_player_persistent_data(uuid, Tag::Compound(data))
}

// ---- screen options (`PlayerOptions`) ----------------------------------------------------

/// The accessories screen's options for a player, kept on the server
/// (`PlayerOptions`; unset ones fall back to the config's or the defaults).
#[derive(Clone, Debug, PartialEq)]
struct Options {
    /// `EQUIP_CONTROL`: `MUST_NOT_CROUCH`, `MUST_CROUCH` or `DISABLED` (the client's config).
    equip_control: String,
    /// Slots no item fits shown too (`UNUSED_SLOTS`).
    unused_slots: bool,
    /// Cosmetic slots shown (`COSMETIC`).
    cosmetic: bool,
    /// The cosmetic grid beside the accessories' (`SIDE_BY_SIDE_SLOTS`, "Toggle") or instead of it ("Switch").
    side_by_side_slots: bool,
    /// Columns of the accessories grid (`COLUMN_AMOUNT`).
    columns: i32,
    /// `WIDGET_TYPE`: 1 paginated, 2 scrollable.
    widget_type: i32,
    /// The group filter beside the entity (`GROUP_FILTER`).
    group_filter: bool,
    /// Groups the grid is narrowed to (`FILTERED_GROUPS`); none: all.
    filtered_groups: Vec<String>,
    /// `ENTITY_CENTERED`.
    entity_centered: bool,
    /// Two views of the entity (`SIDE_BY_SIDE_ENTITY`).
    side_by_side_entity: bool,
    /// The accessories grid left of the entity (`MAIN_WIDGET_POSITION`).
    main_left: bool,
    /// The group filter beside the grid (`SIDE_WIDGET_POSITION`), else across the entity.
    side_together: bool,
    /// `SHOW_CRAFTING_GRID`.
    crafting: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            equip_control: "MUST_NOT_CROUCH".into(),
            unused_slots: config().show_unused_slots,
            cosmetic: false,
            side_by_side_slots: false,
            columns: 2,
            widget_type: 2,
            group_filter: true,
            filtered_groups: Vec::new(),
            entity_centered: true,
            side_by_side_entity: false,
            main_left: true,
            side_together: false,
            crafting: false,
        }
    }
}

impl Options {
    /// `name=value` pairs, `|` between them.
    fn encode(&self) -> String {
        let b = |v: bool| if v { "1" } else { "0" };
        [
            format!("equip_control={}", self.equip_control),
            format!("unused_slots={}", b(self.unused_slots)),
            format!("cosmetic={}", b(self.cosmetic)),
            format!("side_by_side_slots={}", b(self.side_by_side_slots)),
            format!("column_amount={}", self.columns),
            format!("widget_type={}", self.widget_type),
            format!("group_filter={}", b(self.group_filter)),
            format!("filtered_groups={}", self.filtered_groups.join(",")),
            format!("entity_centered={}", b(self.entity_centered)),
            format!("side_by_side_entity={}", b(self.side_by_side_entity)),
            format!("main_widget_position={}", b(self.main_left)),
            format!("side_widget_position={}", b(self.side_together)),
            format!("show_crafting_grid={}", b(self.crafting)),
        ]
        .join("|")
    }
    fn decode(text: &str) -> Self {
        let mut o = Self::default();
        for pair in text.split('|') {
            if let Some((k, v)) = pair.split_once('=') {
                o.set(k, v);
            }
        }
        o
    }
    /// `name=value` changes it (a client's `SyncOptionChange`).
    fn set(&mut self, name: &str, value: &str) {
        let on = value == "1" || value == "true";
        match name {
            "equip_control" if matches!(value, "MUST_NOT_CROUCH" | "MUST_CROUCH" | "DISABLED") => self.equip_control = value.to_owned(),
            "unused_slots" => self.unused_slots = on,
            "cosmetic" => self.cosmetic = on,
            "side_by_side_slots" => self.side_by_side_slots = on,
            "column_amount" => self.columns = value.parse().unwrap_or(2).clamp(1, 9),
            "widget_type" => self.widget_type = value.parse().unwrap_or(2).clamp(1, 2),
            "group_filter" => self.group_filter = on,
            "filtered_groups" => self.filtered_groups = value.split(',').filter(|g| !g.is_empty()).map(str::to_owned).collect(),
            "entity_centered" => self.entity_centered = on,
            "side_by_side_entity" => self.side_by_side_entity = on,
            "main_widget_position" => self.main_left = on,
            "side_widget_position" => self.side_together = on,
            "show_crafting_grid" => self.crafting = on,
            _ => {}
        }
    }
}

fn read_options(game: &Game<'_>, uuid: u128) -> Options {
    match game.native_player_persistent_data(uuid) {
        Some(Tag::Compound(c)) => match c.get(&jkey(OPTIONS)) {
            Some(Tag::String(text)) => Options::decode(&s(text)),
            _ => Options::default(),
        },
        _ => Options::default(),
    }
}

fn write_options(game: &mut Game<'_>, uuid: u128, options: &Options) {
    let mut data = match game.native_player_persistent_data(uuid) {
        Some(Tag::Compound(c)) => c,
        Some(_) => Compound::new(),
        None => return,
    };
    data.insert(jkey(OPTIONS), Tag::String(jkey(&options.encode())));
    let _ = game.native_set_player_persistent_data(uuid, Tag::Compound(data));
}

// ---- the content options (`AccessoriesConfigModel.contentOptions`) -----------------------

/// Where the config allows banners, gliders and totems.
#[derive(Clone, Debug, PartialEq)]
struct ContentOptions {
    allow_banner_equip: bool,
    valid_banner_slots: Vec<String>,
    allow_glider_equip: bool,
    valid_glider_slots: Vec<String>,
    allow_totem_equip: bool,
    valid_totem_slots: Vec<String>,
}

/// The mod's config (`AccessoriesConfigModel`, `config/accessories.json5`).
#[derive(Clone, Debug, PartialEq)]
struct Config {
    content: ContentOptions,
    /// `clientOptions.equipControl`: `MUST_NOT_CROUCH`, `MUST_CROUCH` or `DISABLED`.
    equip_control: String,
    disable_empty_slot_screen_error: bool,
    show_cosmetic_accessories: bool,
    /// `disabledDefaultRenders`: slot type and `ITEM`, `BLOCK` or `ALL`.
    disabled_default_renders: Vec<(String, String)>,
    prioritize_creative_screen: bool,
    /// `menuButtonInjections`: menu type and offset.
    menu_button_injections: Vec<(String, f32, f32)>,
    keybind_ignores_other_targets: bool,
    back_button_closes_screen: bool,
    show_unused_slots: bool,
    allow_slot_scrolling: bool,
    is_dark_mode: bool,
    show_equipped_stack_slot_type: bool,
    entity_looks_at_mouse_cursor: bool,
    always_show_crafting_grid: bool,
    /// `equipCheckTooltipType`: `ALL`, `ADVANCED`, `BASIC` or `DISABLED`.
    equip_check_tooltip_type: String,
    show_slot_darkening_effect: bool,
    /// `modifiers`: a slot type's amount changed by this much.
    modifiers: Vec<(String, i64)>,
}

impl Default for Config {
    fn default() -> Self {
        let list = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect();
        Self {
            content: ContentOptions {
                allow_banner_equip: true,
                valid_banner_slots: list(&["cape", "hat"]),
                allow_glider_equip: false,
                valid_glider_slots: list(&["cape", "back"]),
                allow_totem_equip: false,
                valid_totem_slots: list(&["charm"]),
            },
            equip_control: "MUST_NOT_CROUCH".into(),
            disable_empty_slot_screen_error: false,
            show_cosmetic_accessories: true,
            disabled_default_renders: Vec::new(),
            prioritize_creative_screen: false,
            menu_button_injections: vec![
                ("minecraft:creative_player_inventory".into(), 96.0, 6.0),
                ("minecraft:player_inventory".into(), 66.0, 8.0),
                ("minecraft:horse_inventory".into(), 69.0, 18.0),
            ],
            keybind_ignores_other_targets: false,
            back_button_closes_screen: false,
            show_unused_slots: false,
            allow_slot_scrolling: true,
            is_dark_mode: false,
            show_equipped_stack_slot_type: true,
            entity_looks_at_mouse_cursor: false,
            always_show_crafting_grid: false,
            equip_check_tooltip_type: "BASIC".into(),
            show_slot_darkening_effect: true,
            modifiers: Vec::new(),
        }
    }
}

/// JSON5 as owo-config writes it (comments, trailing commas, bare keys) as JSON.
fn json5_to_json(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' || c == '\'' {
            // A string: copied, single quotes made double.
            let quote = c;
            out.push('"');
            i += 1;
            while i < chars.len() && chars[i] != quote {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    out.push(chars[i]);
                    out.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                if chars[i] == '"' {
                    out.push('\\');
                }
                out.push(chars[i]);
                i += 1;
            }
            out.push('"');
            i += 1;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if c == ',' {
            // A comma before a closing bracket goes.
            let mut k = i + 1;
            while k < chars.len() && chars[k].is_whitespace() {
                k += 1;
            }
            if !matches!(chars.get(k), Some('}' | ']')) {
                out.push(c);
            }
            i += 1;
        } else if c.is_ascii_alphabetic() || c == '_' {
            // A bare word: a key (quoted), or true/false/null.
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let mut k = i;
            while k < chars.len() && chars[k].is_whitespace() {
                k += 1;
            }
            if chars.get(k) == Some(&':') {
                out.push_str(&format!("\"{word}\""));
            } else {
                out.push_str(&word);
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

impl Config {
    fn from_json(v: &serde_json::Value) -> Self {
        let mut c = Self::default();
        let b = |v: &serde_json::Value, d: bool| v.as_bool().unwrap_or(d);
        let strings = |v: &serde_json::Value, d: &Vec<String>| {
            v.as_array().map_or_else(|| d.clone(), |a| a.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
        };
        let content = &v["contentOptions"];
        c.content.valid_glider_slots = strings(&content["validGliderSlots"], &c.content.valid_glider_slots);
        c.content.allow_glider_equip = b(&content["allowGliderEquip"], c.content.allow_glider_equip);
        c.content.valid_totem_slots = strings(&content["validTotemSlots"], &c.content.valid_totem_slots);
        c.content.allow_totem_equip = b(&content["allowTotemEquip"], c.content.allow_totem_equip);
        c.content.valid_banner_slots = strings(&content["validBannerSlots"], &c.content.valid_banner_slots);
        c.content.allow_banner_equip = b(&content["allowBannerEquip"], c.content.allow_banner_equip);
        let client = &v["clientOptions"];
        if let Some(e) = client["equipControl"].as_str() {
            c.equip_control = e.to_ascii_uppercase();
        }
        c.disable_empty_slot_screen_error = b(&client["disableEmptySlotScreenError"], false);
        c.show_cosmetic_accessories = b(&client["showCosmeticAccessories"], true);
        if let Some(list) = client["disabledDefaultRenders"].as_array() {
            c.disabled_default_renders = list
                .iter()
                .filter_map(|e| Some((e["slotType"].as_str()?.to_owned(), e["targetType"].as_str().unwrap_or("ALL").to_ascii_uppercase())))
                .collect();
        }
        let screen = &v["screenOptions"];
        c.prioritize_creative_screen = b(&screen["prioritizeCreativeScreen"], false);
        if let Some(list) = screen["menuButtonInjections"].as_array() {
            c.menu_button_injections = list
                .iter()
                .filter_map(|e| {
                    let menu = e["menuType"].as_str()?;
                    let menu = if menu.contains(':') { menu.to_owned() } else { format!("minecraft:{menu}") };
                    Some((menu, e["xOffset"].as_f64().unwrap_or(0.0) as f32, e["yOffset"].as_f64().unwrap_or(0.0) as f32))
                })
                .collect();
        }
        c.keybind_ignores_other_targets = b(&screen["keybindIgnoresOtherTargets"], false);
        c.back_button_closes_screen = b(&screen["backButtonClosesScreen"], false);
        c.show_unused_slots = b(&screen["showUnusedSlots"], false);
        c.allow_slot_scrolling = b(&screen["allowSlotScrolling"], true);
        c.is_dark_mode = b(&screen["isDarkMode"], false);
        c.show_equipped_stack_slot_type = b(&screen["showEquippedStackSlotType"], true);
        c.always_show_crafting_grid = b(&screen["alwaysShowCraftingGrid"], false);
        c.entity_looks_at_mouse_cursor = b(&screen["entityLooksAtMouseCursor"], false);
        if let Some(t) = screen["equipCheckTooltipType"].as_str() {
            c.equip_check_tooltip_type = t.to_ascii_uppercase();
        }
        c.show_slot_darkening_effect = b(&screen["showSlotDarkeningEffect"], true);
        if let Some(list) = v["modifiers"].as_array() {
            c.modifiers = list.iter().filter_map(|e| Some((e["slotType"].as_str()?.to_owned(), e["amount"].as_i64().unwrap_or(0)))).collect();
        }
        c
    }

    /// The file as owo-config writes it.
    fn to_json5(&self) -> String {
        let list = |v: &[String]| serde_json::to_string(v).unwrap_or_else(|_| "[]".into());
        let renders: Vec<serde_json::Value> =
            self.disabled_default_renders.iter().map(|(s, t)| serde_json::json!({"slotType": s, "targetType": t})).collect();
        let buttons: Vec<serde_json::Value> = self
            .menu_button_injections
            .iter()
            .map(|(m, x, y)| serde_json::json!({"menuType": m, "xOffset": *x as i64, "yOffset": *y as i64}))
            .collect();
        let modifiers: Vec<serde_json::Value> = self.modifiers.iter().map(|(s, a)| serde_json::json!({"slotType": s, "amount": a})).collect();
        let v = serde_json::json!({
            "contentOptions": {
                "validGliderSlots": serde_json::from_str::<serde_json::Value>(&list(&self.content.valid_glider_slots)).unwrap_or_default(),
                "allowGliderEquip": self.content.allow_glider_equip,
                "validTotemSlots": serde_json::from_str::<serde_json::Value>(&list(&self.content.valid_totem_slots)).unwrap_or_default(),
                "allowTotemEquip": self.content.allow_totem_equip,
                "validBannerSlots": serde_json::from_str::<serde_json::Value>(&list(&self.content.valid_banner_slots)).unwrap_or_default(),
                "allowBannerEquip": self.content.allow_banner_equip,
            },
            "clientOptions": {
                "equipControl": self.equip_control,
                "forceNullRenderReplacement": false,
                "disableEmptySlotScreenError": self.disable_empty_slot_screen_error,
                "showCosmeticAccessories": self.show_cosmetic_accessories,
                "disabledDefaultRenders": renders,
            },
            "screenOptions": {
                "prioritizeCreativeScreen": self.prioritize_creative_screen,
                "menuButtonInjections": buttons,
                "defaultValues": null,
                "keybindIgnoresOtherTargets": self.keybind_ignores_other_targets,
                "backButtonClosesScreen": self.back_button_closes_screen,
                "showUnusedSlots": self.show_unused_slots,
                "allowSlotScrolling": self.allow_slot_scrolling,
                "isDarkMode": self.is_dark_mode,
                "showEquippedStackSlotType": self.show_equipped_stack_slot_type,
                "entityLooksAtMouseCursor": self.entity_looks_at_mouse_cursor,
                "alwaysShowCraftingGrid": self.always_show_crafting_grid,
                "equipCheckTooltipType": self.equip_check_tooltip_type,
                "showSlotDarkeningEffect": self.show_slot_darkening_effect,
                "hoveredOptions": {"brightenHovered": true, "cycleBrightness": true, "line": false, "clickbait": false},
                "unHoveredOptions": {"renderUnHovered": true, "darkenUnHovered": true, "darkenedBrightness": 0.5, "darkenedOpacity": 1.0},
            },
            "modifiers": modifiers,
        });
        serde_json::to_string_pretty(&v).unwrap_or_default() + "\n"
    }
}

fn config_path() -> Option<std::path::PathBuf> {
    Game::native_config_dir().ok().map(|d| d.join("accessories.json5"))
}

static CONFIG: std::sync::RwLock<Option<Config>> = std::sync::RwLock::new(None);

/// The config, read once (written with its defaults when there is none).
fn config() -> Config {
    if let Some(c) = CONFIG.read().ok().and_then(|c| c.clone()) {
        return c;
    }
    let path = config_path();
    let read = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok());
    let c = match read.as_deref().and_then(|t| serde_json::from_str::<serde_json::Value>(&json5_to_json(t)).ok()) {
        Some(v) => Config::from_json(&v),
        None => {
            let c = Config::default();
            if read.is_none()
                && let Some(p) = &path
            {
                let _ = std::fs::create_dir_all(p.parent().unwrap_or(p));
                let _ = std::fs::write(p, c.to_json5());
            }
            c
        }
    };
    if let Ok(mut w) = CONFIG.write() {
        *w = Some(c.clone());
    }
    c
}

/// Changes the config and saves it (`/accessories feature`, the screen's theme).
fn change_config(f: impl FnOnce(&mut Config)) {
    let mut c = config();
    f(&mut c);
    if let Some(p) = config_path() {
        let _ = std::fs::write(p, c.to_json5());
    }
    if let Ok(mut w) = CONFIG.write() {
        *w = Some(c);
    }
}

fn content_options() -> ContentOptions {
    config().content
}

/// `isValidSlotWithAny`.
fn in_slots(list: &[String], slot: &str) -> bool {
    list.iter().any(|s| s == slot || s == "any")
}

/// The rule menu slots of slot type `t` follow.
fn rule_of(t: &SlotType, cosmetic: bool) -> menu::SlotRule {
    let c = content_options();
    menu::SlotRule {
        kind: menu::Kind::Accessory,
        slot: t.name.clone(),
        cosmetic,
        validators: t.validators.clone(),
        banner: c.allow_banner_equip && in_slots(&c.valid_banner_slots, &t.name),
        glider: c.allow_glider_equip && in_slots(&c.valid_glider_slots, &t.name),
        totem: c.allow_totem_equip && in_slots(&c.valid_totem_slots, &t.name),
        unique: t.unique,
    }
}

// ---- the server ------------------------------------------------------------------------

/// What a menu slot is, for writing the menu back.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Entry {
    /// Slot type, index, cosmetic.
    Slot(String, usize, bool),
    Armor(usize),
    Offhand,
}

/// An open accessories menu: its window, what each slot is, and what its
/// slots held when last seen (the armour and off hand slots mirror what the
/// player wears: a change on either side is copied to the other).
#[derive(Clone, Debug)]
struct Open {
    window: i32,
    /// Whose accessories and armour the menu holds (the viewer's own, or
    /// another player's through `/accessories edit`).
    owner: u128,
    entries: Vec<Entry>,
    last: Vec<Option<ItemStack>>,
    /// The occupied slots last sent in the data slots (16 a word).
    mask: Vec<i32>,
}

/// Which of a menu's slots hold something, 16 a data slot.
fn occupied_mask(items: &[Option<ItemStack>]) -> Vec<i32> {
    let mut mask = vec![0i32; 16];
    for (i, s) in items.iter().enumerate().take(256) {
        if s.is_some() {
            mask[i / 16] |= 1 << (i % 16);
        }
    }
    mask
}

#[derive(Default)]
struct State {
    /// Players whose menu opens at the next tick.
    opening: Vec<(u128, u128)>,
    open: HashMap<u128, Open>,
    /// What closed menus held, to write back.
    closed: Vec<(u128, i32, Vec<Option<ItemStack>>)>,
    /// Players who joined, respawned or changed dimension: told and told about.
    joined: Vec<u128>,
    /// Players who died, in which dimension, where.
    died: Vec<(u128, String, [f64; 3])>,
    /// Players who used an item that may go on (`attemptEquipFromUse`).
    equip: Vec<(u128, ItemStack)>,
    /// Render toggles asked: player, menu slot.
    toggles: Vec<(u128, usize)>,
    /// Option changes asked: player, name, value.
    options: Vec<(u128, String, String)>,
    /// The data was loaded again: every player is checked.
    reloaded: bool,
    /// What each player was last told to wear, so the others are told of changes.
    told: HashMap<u128, String>,
    /// The slots changed (a data load): every client is told them again.
    slots_dirty: bool,
    /// Players a totem in their accessory slots saved, to use it up.
    totems: Vec<u128>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static LAST_TICK: Cell<Option<i64>> = const { Cell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// The first free slot of a guest's inventory, the hotbar first.
fn free_slot(inventory: &[Option<ItemStack>]) -> Option<usize> {
    inventory.iter().take(36).position(Option::is_none)
}

/// `stack` into player `uuid`'s inventory, else dropped at them.
fn give_back(game: &mut Game<'_>, uuid: u128, stack: ItemStack) {
    let left = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)).and_then(|g| match free_slot(&g.inventory) {
        Some(k) => {
            g.inventory[k] = Some(stack.clone());
            None
        }
        None => Some((stack.clone(), g.position)),
    });
    if let Some((stack, at)) = left {
        game.spawn_item(stack, at, [0.0; 3], 40);
    }
}

/// The menu's slots, in its order: the armour, the off hand, the cosmetic
/// armour, then each shown slot's [cosmetic, accessory] pairs.
fn menu_layout(holder: &Holder) -> (Vec<Entry>, Vec<menu::SlotRule>) {
    let mut entries = Vec::new();
    let mut rules = Vec::new();
    for k in 0..4 {
        entries.push(Entry::Armor(k));
        rules.push(menu::SlotRule { kind: menu::Kind::Armor(k), ..Default::default() });
    }
    entries.push(Entry::Offhand);
    rules.push(menu::SlotRule { kind: menu::Kind::Offhand, ..Default::default() });
    with_data(|d| {
        for part in &COSMETIC_ARMOR[..4] {
            let name = format!("accessories_cosmetics:{part}");
            if let Some(t) = d.slot(&name).filter(|_| holder.contains_key(&name)) {
                entries.push(Entry::Slot(name.clone(), 0, false));
                rules.push(rule_of(t, false));
            }
        }
        for t in d.shown_slots() {
            let size = holder.get(&t.name).map_or(0, |c| c.items.len());
            for i in 0..size {
                entries.push(Entry::Slot(t.name.clone(), i, true));
                rules.push(rule_of(t, true));
                entries.push(Entry::Slot(t.name.clone(), i, false));
                rules.push(rule_of(t, false));
            }
        }
    });
    (entries, rules)
}

/// What menu slot `e` holds now.
fn entry_stack(holder: &Holder, armor: &[Option<ItemStack>; 4], offhand: &Option<ItemStack>, e: &Entry) -> Option<ItemStack> {
    match e {
        Entry::Armor(k) => armor[*k].clone(),
        Entry::Offhand => offhand.clone(),
        Entry::Slot(name, i, cosmetic) => {
            let c = holder.get(name)?;
            if *cosmetic { c.cosmetics.get(*i).cloned().flatten() } else { c.items.get(*i).cloned().flatten() }
        }
    }
}

/// Whether any item at all fits slot type `t` (`getUsedSlotsFor`'s registry
/// pass): the grid hides the others unless unused slots are shown.
fn slot_used(items: &horizon_client::items::Items, t: &SlotType) -> bool {
    let rule = rule_of(t, false);
    items.order.iter().any(|id| menu::valid(&rule, &ItemStack::new(id, 1), items))
}

/// The screen's layout, sent ahead of the menu: the options, the groups, and
/// what each menu slot is.
fn layout_text(game: &Game<'_>, holder: &Holder, entries: &[Entry], options: &Options, carried: &[ItemStack]) -> String {
    let items = game.items();
    let mut out = format!("accessories 1\nopt\t{}\n", options.encode());
    with_data(|d| {
        for g in d.groups.iter().filter(|g| !g.unique) {
            if g.slots.iter().any(|s| d.player.contains(s)) {
                out.push_str(&format!("group\t{}\t{}\n", g.name, icon_sprite(&g.icon)));
            }
        }
        let mut used: HashMap<String, bool> = HashMap::new();
        for e in entries {
            match e {
                Entry::Armor(k) => out.push_str(&format!("slot\tarmor\t{k}\n")),
                Entry::Offhand => out.push_str("slot\toffhand\t0\n"),
                Entry::Slot(name, i, cosmetic) => {
                    let Some(t) = d.slot(name) else {
                        out.push_str("slot\tnone\t0\n");
                        continue;
                    };
                    let render = holder.get(name).and_then(|c| c.render.get(*i).copied()).unwrap_or(true);
                    if t.unique {
                        out.push_str(&format!("slot\tcosarmor\t{}\t{}\n", name, u8::from(render)));
                        continue;
                    }
                    let u = *used.entry(name.clone()).or_insert_with(|| {
                        slot_used(items, t)
                            || holder.get(name).is_some_and(|c| c.items.iter().chain(&c.cosmetics).any(Option::is_some))
                            || carried.iter().any(|s| menu::valid(&rule_of(t, false), s, items))
                    });
                    let group = d.group_of(name).map_or("unsorted", |g| g.name.as_str());
                    out.push_str(&format!(
                        "slot\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                        if *cosmetic { "cosmetic" } else { "accessory" },
                        name,
                        i,
                        group,
                        icon_sprite(&t.icon),
                        u8::from(render),
                        u8::from(u)
                    ));
                }
            }
        }
    });
    out
}

/// Opens player `owner`'s accessories menu (`AccessoriesMenu`) for player `uuid`.
fn open_menu(game: &mut Game<'_>, uuid: u128, owner: u128) {
    if with(|s| s.open.contains_key(&uuid)) {
        return;
    }
    let Some((holder, invalid)) = read_holder(game, owner) else {
        return;
    };
    for stack in invalid {
        give_back(game, owner, stack);
    }
    write_holder(game, owner, &holder);
    let options = read_options(game, uuid);
    let (entries, rules) = menu_layout(&holder);
    let Some((armor, offhand)) = game
        .lan
        .as_ref()
        .and_then(|h| h.hub.guests.iter().find(|g| g.placed && g.uuid == owner))
        .map(|g| (g.armor.clone(), g.offhand.clone()))
    else {
        return;
    };
    let Some((conn, carried)) = game
        .lan
        .as_ref()
        .and_then(|h| h.hub.guests.iter().find(|g| g.placed && g.uuid == uuid))
        .map(|g| (g.conn, g.inventory.iter().take(36).flatten().cloned().collect::<Vec<_>>()))
    else {
        return;
    };
    let mut items: Vec<Option<ItemStack>> = entries.iter().map(|e| entry_stack(&holder, &armor, &offhand, e)).collect();
    let layout = layout_text(game, &holder, &entries, &options, &carried);
    let title = horizon_client::lan::data::component_nbt(&Text::literal("Accessories").component());
    let Some(host) = game.lan.as_mut() else {
        return;
    };
    let shown = items.clone();
    items.resize(menu::MENU_SLOTS, None);
    if let Some(window) = menu::open(&mut host.hub, conn, MENU, title, shown, layout.as_bytes(), rules) {
        with(|s| {
            s.open.insert(uuid, Open { window, owner, entries, last: items, mask: Vec::new() });
        });
    }
}

/// Each tick, an open menu's armour and off hand slots and what the player
/// wears follow each other (the side that changed since last seen wins), and
/// its accessories are written to the holder when they changed.
fn sync_open(game: &mut Game<'_>, uuid: u128) {
    let Some(mut open) = with(|s| s.open.get(&uuid).cloned()) else {
        return;
    };
    let Some(host) = game.lan.as_mut() else {
        return;
    };
    let Some(conn) = host.hub.guests.iter().find(|g| g.uuid == uuid).map(|g| g.conn) else {
        return;
    };
    let Some(mut items) = menu::items(&host.hub, conn, open.window) else {
        return;
    };
    let mut resend = false;
    if let Some(g) = host.hub.guests.iter_mut().find(|g| g.uuid == open.owner) {
        for (j, e) in open.entries.iter().enumerate() {
            let worn = match e {
                Entry::Armor(k) => &mut g.armor[*k],
                Entry::Offhand => &mut g.offhand,
                Entry::Slot(..) => continue,
            };
            let (Some(now), Some(was)) = (items.get(j).cloned(), open.last.get(j).cloned()) else {
                continue;
            };
            if now != was {
                *worn = now;
            } else if *worn != now {
                items[j] = worn.clone();
                resend = true;
            }
        }
    }
    if resend {
        menu::set_items(&mut host.hub, conn, open.window, items.clone());
    }
    let mask = occupied_mask(&items);
    if mask != open.mask {
        menu::send_data(&mut host.hub, conn, open.window, &mask);
        open.mask = mask.clone();
        with(|s| {
            if let Some(o) = s.open.get_mut(&uuid) {
                o.mask = mask;
            }
        });
    }
    if items != open.last {
        write_back(game, open.owner, &open, &items);
        open.last = items;
        with(|s| {
            if let Some(o) = s.open.get_mut(&uuid) {
                o.last = open.last;
            }
        });
    }
}

/// The accessories an open (or just closed) menu holds go into the holder.
fn write_back(game: &mut Game<'_>, uuid: u128, open: &Open, items: &[Option<ItemStack>]) {
    let Some((mut holder, invalid)) = read_holder(game, uuid) else {
        return;
    };
    for stack in invalid {
        give_back(game, uuid, stack);
    }
    let before = holder.clone();
    for (e, stack) in open.entries.iter().zip(items) {
        match e {
            Entry::Armor(_) | Entry::Offhand => {}
            Entry::Slot(name, i, cosmetic) => {
                if let Some(c) = holder.get_mut(name) {
                    let list = if *cosmetic { &mut c.cosmetics } else { &mut c.items };
                    if let Some(place) = list.get_mut(*i) {
                        *place = stack.clone();
                    }
                }
            }
        }
    }
    if holder != before {
        // `onEquipItem`: a slot that got a different item plays its equip sound.
        for (name, c) in &holder {
            let Some(old) = before.get(name) else {
                continue;
            };
            for (new, was) in c.items.iter().zip(&old.items).chain(c.cosmetics.iter().zip(&old.cosmetics)) {
                if let Some(stack) = new
                    && was.as_ref().is_none_or(|w| !w.same_item_same_components(stack))
                {
                    equip_sound(game, uuid, stack);
                }
            }
        }
        write_holder(game, uuid, &holder);
        with(|s| s.told.remove(&uuid));
    }
}

/// A closed menu's slots: the accessories go into the holder, and an armour
/// or off hand slot changed since last seen is what the player wears now.
fn closed_menu(game: &mut Game<'_>, open: &Open, items: &[Option<ItemStack>]) {
    let uuid = open.owner;
    if let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)) {
        for (j, e) in open.entries.iter().enumerate() {
            let worn = match e {
                Entry::Armor(k) => &mut g.armor[*k],
                Entry::Offhand => &mut g.offhand,
                Entry::Slot(..) => continue,
            };
            let now = items.get(j).cloned().flatten();
            if open.last.get(j).is_some_and(|was| *was != now) {
                *worn = now;
            }
        }
    }
    write_back(game, uuid, open, items);
}

/// `AccessoriesEventHandler.attemptEquipFromUse`: the stack used goes into the
/// first slot (in the player's slot order) that takes it and allows equipping
/// from use, an empty one first, else swapped with the first occupied one
/// whose accessory may come off. Some(slot, index) when it found one.
fn equip_target(game: &Game<'_>, holder: &Holder, stack: &ItemStack, creative: bool) -> Option<(String, usize)> {
    let items = game.items();
    // `canEquipFromUse`: an equippable item only when swappable.
    if items.get(&stack.item).is_some_and(|i| i.equip_slot.is_some() && !i.swappable) {
        return None;
    }
    with_data(|d| {
        let candidates: Vec<&SlotType> = d.player.iter().filter_map(|n| d.slot(n)).filter(|t| !t.unique).collect();
        for t in &candidates {
            let Some(c) = holder.get(&t.name) else {
                continue;
            };
            if !menu::valid(&rule_of(t, false), stack, items) {
                continue;
            }
            if let Some(i) = c.items.iter().position(Option::is_none) {
                return Some((t.name.clone(), i));
            }
        }
        for t in &candidates {
            let Some(c) = holder.get(&t.name) else {
                continue;
            };
            if !menu::valid(&rule_of(t, false), stack, items) {
                continue;
            }
            let off = |s: &Option<ItemStack>| s.as_ref().is_some_and(|s| creative || s.enchantment_level("minecraft:binding_curse") == 0);
            if let Some(i) = c.items.iter().position(off) {
                return Some((t.name.clone(), i));
            }
        }
        None
    })
}

/// Equips what player `uuid` used (`stack`) from the hand that holds it.
fn equip_from_use(game: &mut Game<'_>, uuid: u128, stack: &ItemStack) {
    if with(|s| s.open.contains_key(&uuid)) {
        return;
    }
    let Some((mut holder, invalid)) = read_holder(game, uuid) else {
        return;
    };
    for s in invalid {
        give_back(game, uuid, s);
    }
    let Some((hand, held, creative)) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.placed && g.uuid == uuid)).and_then(|g| {
        let main = g.inventory.get(g.selected).cloned().flatten();
        if main.as_ref().is_some_and(|m| m.same_item_same_components(stack)) {
            Some((Some(g.selected), main?, g.creative()))
        } else if g.offhand.as_ref().is_some_and(|o| o.same_item_same_components(stack)) {
            Some((None, g.offhand.clone()?, g.creative()))
        } else {
            None
        }
    }) else {
        return;
    };
    let Some((slot, i)) = equip_target(game, &holder, &held, creative) else {
        return;
    };
    // The hand gives as many as the slot holds (`maxStackSize`: one without `stack_settings`).
    let moved = held.count.min(menu::max_in_slot(&held, game.items()));
    let mut one = held.clone();
    one.count = moved;
    let old = holder.get_mut(&slot).and_then(|c| c.items.get_mut(i)).and_then(|place| place.replace(one.clone()));
    if !write_holder(game, uuid, &holder) {
        return;
    }
    // The hand gives one; what came off goes into it, else into the inventory.
    let mut rest = held;
    rest.count -= moved;
    let mut spare = None;
    if let Some(g) = game.lan.as_mut().and_then(|h| h.hub.guests.iter_mut().find(|g| g.uuid == uuid)) {
        let place = match hand {
            Some(k) => &mut g.inventory[k],
            None => &mut g.offhand,
        };
        if rest.count > 0 {
            *place = Some(rest);
            spare = old;
        } else {
            *place = old;
        }
    }
    if let Some(s) = spare {
        give_back(game, uuid, s);
    }
    equip_sound(game, uuid, &one);
    with(|s| s.told.remove(&uuid));
}

/// `AccessoriesEventHandler.onDeath`: each accessory and cosmetic goes by its
/// slot's drop rule; `DEFAULT` keeps it with keepInventory, destroys a
/// vanishing-cursed one, and drops the rest where the player died.
fn drop_on_death(game: &mut Game<'_>, uuid: u128, at: [f64; 3]) {
    let Some((mut holder, invalid)) = read_holder(game, uuid) else {
        return;
    };
    let keep = game.ui.rules.bool("keep_inventory");
    let mut dropped: Vec<ItemStack> = invalid;
    with_data(|d| {
        for (name, c) in holder.iter_mut() {
            let rule = d.slot(name).map_or(DropRule::Default, |t| t.drop_rule);
            for place in c.items.iter_mut().chain(c.cosmetics.iter_mut()) {
                let Some(stack) = place.as_ref() else {
                    continue;
                };
                match rule {
                    DropRule::Keep => {}
                    DropRule::Destroy => *place = None,
                    DropRule::Drop => dropped.extend(place.take()),
                    DropRule::Default => {
                        if keep {
                        } else if stack.enchantment_level("minecraft:vanishing_curse") > 0 {
                            *place = None;
                        } else {
                            dropped.extend(place.take());
                        }
                    }
                }
            }
        }
    });
    write_holder(game, uuid, &holder);
    with(|s| s.told.remove(&uuid));
    // `Player.drop(stack, true, false)`: from the eyes, thrown a little in a random way.
    let mut seed = (game.world.game_time as u64) ^ (uuid as u64);
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as f64) / f64::from(u32::MAX >> 1)
    };
    for stack in dropped {
        let power = random() * 0.5;
        let angle = random() * std::f64::consts::TAU;
        let velocity = [-angle.sin() * power, 0.2, angle.cos() * power];
        game.spawn_item(stack, [at[0], at[1] + 1.32, at[2]], velocity, 40);
    }
}

/// What player `uuid` shows on their model: each slot's cosmetic, else its
/// accessory, where its render toggle is on (`AccessoriesRenderLayer`), as
/// `slot\tindex\tSNBT` lines.
fn worn_text(game: &Game<'_>, uuid: u128) -> String {
    let Some((holder, _)) = read_holder(game, uuid) else {
        return String::new();
    };
    let mut out = String::new();
    for (name, c) in &holder {
        if name.contains(':') {
            continue;
        }
        for i in 0..c.items.len() {
            if !c.render.get(i).copied().unwrap_or(true) {
                continue;
            }
            let snbt = |s: &Option<ItemStack>| s.as_ref().map(|s| horizon_world::snbt_text::to_snbt(&horizon_items::item_nbt::item_to_nbt(s))).unwrap_or_default();
            let (item, cosmetic) = (c.items.get(i).cloned().flatten(), c.cosmetics.get(i).cloned().flatten());
            if item.is_none() && cosmetic.is_none() {
                continue;
            }
            out.push_str(&format!("{name}\t{i}\t{}\t{}\n", snbt(&cosmetic), snbt(&item)));
        }
    }
    out
}

/// Tells every player what each wears when it changed (`SyncEntireContainer`).
fn tell_worn(game: &mut Game<'_>) {
    let Some(host) = game.lan.as_ref() else {
        return;
    };
    let players: Vec<(u128, String)> = host.hub.guests.iter().filter(|g| g.placed).map(|g| (g.uuid, g.name.clone())).collect();
    for (uuid, name) in &players {
        let text = worn_text(game, *uuid);
        if with(|s| s.told.get(uuid) == Some(&text)) {
            continue;
        }
        let message = format!("{name}\n{text}");
        for (to, _) in &players {
            let _ = game.native_net_send_to_player(*to, WORN_CHANNEL, message.as_bytes());
        }
        with(|s| s.told.insert(*uuid, text));
    }
}

/// Each level's tick, with the hub in hand.
pub(super) fn server_tick(game: &mut Game<'_>) {
    let now = game.world.game_time;
    let first = LAST_TICK.with(|t| t.replace(Some(now)) != Some(now));
    // Closed menus first: what they held is the player's before anything else.
    for (uuid, window, items) in with(|s| std::mem::take(&mut s.closed)) {
        let Some(open) = with(|s| s.open.get(&uuid).cloned()) else {
            continue;
        };
        if open.window != window {
            continue;
        }
        with(|s| s.open.remove(&uuid));
        if let Some(conn) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.conn) {
            menu::forget(conn);
        }
        closed_menu(game, &open, &items);
        // The armour and off hand may have changed under the menu: the
        // inventory menu (window 0) is told again, as it is on closing.
        for who in [uuid, open.owner] {
            if let Some(host) = game.lan.as_mut()
                && let Some(i) = host.hub.guests.iter().position(|g| g.uuid == who)
            {
                host.hub.send_inventory(i);
            }
        }
    }
    // Deaths in this level.
    let dimension = game.dim.id().to_owned();
    let died: Vec<(u128, [f64; 3])> = with(|s| {
        let (here, elsewhere): (Vec<_>, Vec<_>) = std::mem::take(&mut s.died).into_iter().partition(|d| d.1 == dimension);
        s.died = elsewhere;
        here.into_iter().map(|(u, _, at)| (u, at)).collect()
    });
    for (uuid, at) in died {
        let at = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map_or(at, |g| g.position);
        drop_on_death(game, uuid, at);
    }
    if !first {
        return;
    }
    write_mod_menu_files(game);
    if with(|s| std::mem::take(&mut s.reloaded)) {
        // `revalidatePlayer`: what no longer fits goes back to each player.
        let players: Vec<u128> = game.lan.as_ref().map(|h| h.hub.guests.iter().filter(|g| g.placed).map(|g| g.uuid).collect()).unwrap_or_default();
        for uuid in players {
            if let Some((holder, invalid)) = read_holder(game, uuid) {
                for s in invalid {
                    give_back(game, uuid, s);
                }
                write_holder(game, uuid, &holder);
            }
        }
        with(|s| {
            s.told.clear();
            s.slots_dirty = true;
        });
    }
    let joined = with(|s| std::mem::take(&mut s.joined));
    if !joined.is_empty() || with(|s| std::mem::take(&mut s.slots_dirty)) {
        with(|s| s.told.clear());
        // A respawned player's body is new: their modifiers go on again.
        APPLIED.with(|a| {
            let mut a = a.borrow_mut();
            for u in &joined {
                a.remove(u);
            }
        });
        let players: Vec<u128> = game.lan.as_ref().map(|h| h.hub.guests.iter().filter(|g| g.placed).map(|g| g.uuid).collect()).unwrap_or_default();
        tell_slots(game, &players);
    }
    run_commands(game);
    for uuid in with(|s| std::mem::take(&mut s.totems)) {
        use_totem(game, uuid);
    }
    for (uuid, name, value) in with(|s| std::mem::take(&mut s.options)) {
        let mut options = read_options(game, uuid);
        options.set(&name, &value);
        write_options(game, uuid, &options);
    }
    for (uuid, slot) in with(|s| std::mem::take(&mut s.toggles)) {
        let Some((Entry::Slot(name, i, _), owner)) = with(|s| s.open.get(&uuid).and_then(|o| Some((o.entries.get(slot).cloned()?, o.owner)))) else {
            continue;
        };
        let uuid = owner;
        if let Some((mut holder, _)) = read_holder(game, uuid)
            && let Some(r) = holder.get_mut(&name).and_then(|c| c.render.get_mut(i))
        {
            *r = !*r;
            write_holder(game, uuid, &holder);
        }
    }
    for (uuid, stack) in with(|s| std::mem::take(&mut s.equip)) {
        equip_from_use(game, uuid, &stack);
    }
    let open: Vec<u128> = with(|s| s.open.keys().copied().collect());
    for uuid in open {
        sync_open(game, uuid);
    }
    for (uuid, owner) in with(|s| std::mem::take(&mut s.opening)) {
        open_menu(game, uuid, owner);
    }
    tell_worn(game);
    apply_modifiers(game);
    apply_mob_effects(game);
}

/// A menu closed: an accessories menu's slots are taken (or Horizon would
/// give them to the player) and written back at the tick.
pub(super) fn menu_closed(uuid: u128, window: i32, slots: &mut [Option<ItemStack>]) {
    let ours = with(|s| s.open.get(&uuid).is_some_and(|o| o.window == window));
    if ours {
        let items = slots.iter_mut().map(Option::take).collect();
        with(|s| s.closed.push((uuid, window, items)));
    }
}

/// A player's client: open the screen, a render toggle, an option.
pub(super) fn net_from_client(game: &mut Game<'_>, player: u128, channel: &str, data: &[u8]) {
    let _ = game;
    match channel {
        OPEN_CHANNEL => with(|s| {
            if !s.opening.iter().any(|(v, _)| *v == player) {
                s.opening.push((player, player));
            }
        }),
        TOGGLE_CHANNEL => {
            if let [lo, hi] = *data {
                with(|s| s.toggles.push((player, usize::from(u16::from_le_bytes([lo, hi])))));
            }
        }
        OPTION_CHANNEL => {
            if let Some((name, value)) = std::str::from_utf8(data).ok().and_then(|t| t.split_once('=')) {
                with(|s| s.options.push((player, name.to_owned(), value.to_owned())));
            }
        }
        _ => {}
    }
}

/// A player joined, respawned or changed dimension: everyone is told again.
pub(super) fn logged_in(player: u128) {
    with(|s| s.joined.push(player));
}

// ---- item components and attributes (`AccessoriesDataComponents`, `AccessoryAttributeLogic`) ----

/// Accessories' item components (`AccessoriesDataComponents`): items carry
/// them from content mods' data and from `/accessories components`.
pub(super) const COMPONENTS: [&str; 6] = [
    "accessories:slot_validation",
    "accessories:attributes",
    "accessories:stack_settings",
    "accessories:nested_accessories",
    "accessories:mob_effects",
    "accessories:custom_renderer",
];

pub(super) fn component_types(types: &mut Vec<String>) {
    types.extend(COMPONENTS.iter().map(|c| (*c).to_owned()));
}

/// The stacks an accessory holds inside (`accessories:nested_accessories`),
/// counted as worn too (`AccessoryNest`).
fn nested(stack: &ItemStack, items: &horizon_client::items::Items) -> Vec<ItemStack> {
    let Some(Tag::Compound(c)) = menu::component(stack, items, "accessories:nested_accessories") else {
        return Vec::new();
    };
    match c.get(&jkey("accessories")) {
        Some(Tag::List(list)) => list.iter().filter_map(horizon_items::item_nbt::item_from_nbt).filter(|s| s.count > 0).collect(),
        _ => Vec::new(),
    }
}

/// The attribute modifiers a player's worn accessories give
/// (`AccessoryAttributeLogic.getAttributeModifiers`): each slot's stack's
/// `accessories:attributes` for that slot (or any), nested ones included; a
/// stackable modifier's id gets the slot path (`ns:<slot>/<index>/<path>`).
/// Slot sizes (`accessories:<slot>`) are not attributes of the player.
fn worn_modifiers(items: &horizon_client::items::Items, holder: &Holder) -> Vec<(String, String, f64, String)> {
    let mut out: Vec<(String, String, f64, String)> = Vec::new();
    for (name, c) in holder {
        if name.contains(':') {
            continue;
        }
        for (i, stack) in c.items.iter().enumerate() {
            let Some(stack) = stack else {
                continue;
            };
            let mut stacks = vec![(stack.clone(), format!("{name}/{i}"))];
            for (n, inner) in nested(stack, items).into_iter().enumerate() {
                stacks.push((inner, format!("{name}/{i}/nest_0_{n}")));
            }
            for (s, path) in stacks {
                for m in menu::modifiers(&s, items).0 {
                    if (m.slot != *name && m.slot != "any") || menu::is_slot_attribute(&m.attribute) {
                        continue;
                    }
                    let id = if m.stackable {
                        match m.id.split_once(':') {
                            Some((ns, p)) => format!("{ns}:{path}/{p}"),
                            None => format!("minecraft:{path}/{}", m.id),
                        }
                    } else {
                        m.id.clone()
                    };
                    // An exclusive id counts once, as vanilla's modifier map keeps one.
                    if !out.iter().any(|o| o.0 == m.attribute && o.1 == id) {
                        out.push((m.attribute.clone(), id, m.amount, m.operation.clone()));
                    }
                }
            }
        }
    }
    out
}

thread_local! {
    /// The modifiers put on each player, to take off when their accessories change.
    static APPLIED: RefCell<HashMap<u128, Vec<(String, String, f64, String)>>> = RefCell::new(HashMap::new());
}

/// Runs `f` on the player `uuid` as the acting one, where the hub keeps them.
fn as_player<T>(game: &mut Game<'_>, uuid: u128, f: impl FnOnce(&mut Game<'_>) -> T) -> Option<T> {
    let conn = game.lan.as_ref()?.hub.guests.iter().find(|g| g.uuid == uuid && g.placed && !g.dead)?.conn;
    let mut host = game.lan.take()?;
    let mut out = None;
    game.as_guest(&mut host, conn, |g| out = Some(f(g)));
    game.lan = Some(host);
    out
}

/// `AccessoryAttributeLogic`: each player wears their accessories' modifiers
/// (transient), the ones of what came off taken away.
fn apply_modifiers(game: &mut Game<'_>) {
    let players: Vec<u128> = game
        .lan
        .as_ref()
        .map(|h| h.hub.guests.iter().filter(|g| g.placed && !g.dead).map(|g| g.uuid).collect())
        .unwrap_or_default();
    for uuid in players {
        let Some((holder, invalid)) = read_holder(game, uuid) else {
            continue;
        };
        // A slot shrank (its worn modifier came off): what no longer fits goes back.
        if !invalid.is_empty() {
            for s in invalid {
                give_back(game, uuid, s);
            }
            write_holder(game, uuid, &holder);
            with(|s| s.told.remove(&uuid));
        }
        let want = worn_modifiers(game.items(), &holder);
        let had = APPLIED.with(|a| a.borrow().get(&uuid).cloned()).unwrap_or_default();
        if want == had {
            continue;
        }
        let done = as_player(game, uuid, |g| {
            let Some(p) = g.player.as_mut() else {
                return false;
            };
            for (attribute, id, amount, op) in &had {
                if !want.iter().any(|w| w.0 == *attribute && w.1 == *id) {
                    let _ = p.set_modifier(attribute, id, *amount, op, false);
                }
            }
            for (attribute, id, amount, op) in &want {
                let _ = p.set_modifier(attribute, id, *amount, op, true);
            }
            true
        });
        if done == Some(true) {
            APPLIED.with(|a| a.borrow_mut().insert(uuid, want));
        }
    }
}

/// The player's slots for the clients (`SyncedDataHelperManager`): the
/// tooltips of items ask them. `slot\t<name>\t<bound>\t<validators>\t<banner>\t<glider>\t<totem>`.
fn slots_text() -> String {
    with_data(|d| {
        let mut out = String::new();
        let mut names: Vec<&String> = d.player.iter().collect();
        names.extend(d.slots.keys().filter(|n| !d.player.contains(n)));
        for name in names {
            let Some(t) = d.slot(name) else {
                continue;
            };
            if t.unique {
                continue;
            }
            let r = rule_of(t, false);
            out.push_str(&format!(
                "slot\t{}\t{}\t{}\t{}\t{}\t{}\n",
                t.name,
                u8::from(d.player.contains(&t.name)),
                t.validators.join(","),
                u8::from(r.banner),
                u8::from(r.glider),
                u8::from(r.totem)
            ));
        }
        out
    })
}

/// Each player's client learns the slots (on joining, after a data load).
fn tell_slots(game: &mut Game<'_>, players: &[u128]) {
    let text = slots_text();
    for &p in players {
        let _ = game.native_net_send_to_player(p, SLOTS_CHANNEL, text.as_bytes());
    }
}

// The slots the server told: a rule each, and whether the player has it.
thread_local! {
    static CLIENT_SLOTS: RefCell<Vec<(menu::SlotRule, bool)>> = const { RefCell::new(Vec::new()) };
}

fn slots_received(data: &[u8]) {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let mut slots = Vec::new();
    for line in text.lines() {
        let p: Vec<&str> = line.split('\t').collect();
        if p.len() < 7 || p[0] != "slot" {
            continue;
        }
        let rule = menu::SlotRule {
            kind: menu::Kind::Accessory,
            slot: p[1].to_owned(),
            cosmetic: false,
            validators: p[3].split(',').filter(|v| !v.is_empty()).map(str::to_owned).collect(),
            banner: p[4] == "1",
            glider: p[5] == "1",
            totem: p[6] == "1",
            unique: false,
        };
        slots.push((rule, p[2] == "1"));
    }
    CLIENT_SLOTS.with(|c| *c.borrow_mut() = slots);
}

/// A number as vanilla's attribute lines write it (`ItemAttributeModifiers.ATTRIBUTE_MODIFIER_FORMAT`, `#.##`).
fn modifier_number(v: f64) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Vanilla's line for a modifier: `+2 Max Health` in blue, `-10% Speed` in red.
fn modifier_line(game: &Game<'_>, attribute: &str, amount: f64, op: &str) -> Option<String> {
    let shown = match op {
        "add_multiplied_base" | "add_multiplied_total" => amount * 100.0,
        _ => amount,
    };
    if shown == 0.0 {
        return None;
    }
    let k = match op {
        "add_multiplied_base" => 1,
        "add_multiplied_total" => 2,
        _ => 0,
    };
    let path = attribute.split_once(':').map_or(attribute, |(_, p)| p);
    let name = tr(game, &format!("attribute.name.{path}"));
    let (key, colour) = if shown > 0.0 { (format!("attribute.modifier.plus.{k}"), "§9") } else { (format!("attribute.modifier.take.{k}"), "§c") };
    Some(format!("{colour}{}", format_args(&tr(game, &key), &[modifier_number(shown.abs()), name])))
}

/// `AccessoriesEventHandler.getTooltipData`: an item that fits the player's
/// slots says which (`Slot: Hat`, `Slots: Any except …`), and what it gives
/// when worn (`When Equipped:`, or for each slot when they differ).
pub(super) fn tooltip(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    // Over a slot's render toggle the item's tooltip gives way to the toggle.
    if TOGGLE_HOVERED.with(Cell::get) {
        lines.clear();
        return;
    }
    // Over a worn accessory: where it is equipped (`showEquippedStackSlotType`).
    if let Some(slot) = HOVERED_SLOT.with(|h| h.borrow().clone())
        && config().show_equipped_stack_slot_type
        && TOOLTIP_SCREEN.with(Cell::get)
    {
        let name = tr(game, &format!("accessories.slot.{}", slot.replace(':', ".")));
        lines.push((Text::legacy(&format!("§7{}§9{name}", tr(game, "accessories.tooltip.currently_equipped_in"))), u32::MAX));
    }
    let items = game.items();
    let slots = CLIENT_SLOTS.with(|c| c.borrow().clone());
    if slots.is_empty() {
        return;
    }
    let valid: Vec<&menu::SlotRule> = slots.iter().filter(|(r, bound)| *bound && menu::valid(r, stack, items)).map(|(r, _)| r).collect();
    if valid.is_empty() {
        return;
    }
    let name = |slot: &str| tr(game, &format!("accessories.slot.{}", slot.replace(':', ".")));
    let all = slots.iter().all(|(r, _)| valid.iter().any(|v| v.slot == r.slot));
    let mut text = String::new();
    if all {
        text.push_str(&tr(game, "accessories.slot.any"));
    } else {
        let invalid: Vec<&menu::SlotRule> = slots.iter().filter(|(r, bound)| *bound && !valid.iter().any(|v| v.slot == r.slot)).map(|(r, _)| r).collect();
        if invalid.len() < valid.len() {
            text.push_str(&tr(game, "accessories.slot.any"));
            text.push_str(&format!("§7{}", tr(game, "accessories.slot.except")));
            let list: Vec<String> = invalid.iter().map(|r| format!("§c{}", name(&r.slot))).collect();
            text.push_str(&list.join("§7, "));
        } else {
            let list: Vec<String> = valid.iter().map(|r| format!("§9{}", name(&r.slot))).collect();
            text.push_str(&list.join("§7, "));
        }
    }
    let label = if valid.len() > 1 && !all { "accessories.slot.tooltip.plural" } else { "accessories.slot.tooltip.singular" };
    let line = Text::legacy(&format!("§7{}§9{text}", tr(game, label)));
    let at = 1.min(lines.len());
    lines.insert(at, (line, u32::MAX));
    // What it gives in each slot it fits.
    let (mods, shown) = menu::modifiers(stack, items);
    if !shown {
        return;
    }
    let per_slot: Vec<(String, Vec<String>)> = valid
        .iter()
        .map(|r| {
            let lines: Vec<String> = mods
                .iter()
                .filter(|m| (m.slot == r.slot || m.slot == "any") && !menu::is_slot_attribute(&m.attribute))
                .filter_map(|m| modifier_line(game, &m.attribute, m.amount, &m.operation))
                .collect();
            (r.slot.clone(), lines)
        })
        .filter(|(_, l)| !l.is_empty())
        .collect();
    if per_slot.is_empty() {
        return;
    }
    let same = per_slot.iter().all(|(_, l)| *l == per_slot[0].1);
    if same && per_slot.len() == valid.len() {
        lines.push((Text::literal(""), u32::MAX));
        lines.push((Text::legacy(&format!("§7{}", tr(game, "accessories.tooltip.attributes.any"))), u32::MAX));
        for l in &per_slot[0].1 {
            lines.push((Text::legacy(l), u32::MAX));
        }
    } else {
        for (slot, ls) in &per_slot {
            lines.push((Text::literal(""), u32::MAX));
            let head = format_args(&tr(game, "accessories.tooltip.attributes.slot"), &[format!("§9{}§7", name(slot))]);
            lines.push((Text::legacy(&format!("§7{head}")), u32::MAX));
            for l in ls {
                lines.push((Text::legacy(l), u32::MAX));
            }
        }
    }
}

/// A player died (their state swapped in, the hub out of the game's hands):
/// their accessories drop at the tick of the level they died in.
pub(super) fn player_died(game: &Game<'_>, player: u128) {
    // Where they fell is read at the tick, from their record in the hub
    // (still where they died: respawning takes a click).
    let dimension = game.dim.id().to_owned();
    with(|s| s.died.push((player, dimension, [0.0; 3])));
}

/// An item used in the air: an accessory goes on at the tick
/// (`attemptEquipFromUse`, without crouching as `equipControl` says).
pub(super) fn use_item(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if *claimed {
        return;
    }
    let uuid = game.native_acting_id();
    // `equipControl`: without crouching (the default), crouching, or never.
    let sneaking = game.native_sneaking();
    match read_options(game, uuid).equip_control.as_str() {
        "DISABLED" => return,
        "MUST_CROUCH" if !sneaking => return,
        "MUST_NOT_CROUCH" if sneaking => return,
        _ => {}
    }
    let Some((holder, _)) = read_holder(game, uuid) else {
        return;
    };
    if equip_target(game, &holder, stack, false).is_none() {
        return;
    }
    *claimed = true;
    with(|s| {
        if !s.equip.iter().any(|(u, _)| *u == uuid) {
            s.equip.push((uuid, stack.clone()));
        }
    });
}

/// `onEquipItem`'s sound: the equip sound of what went into a slot, heard by those near.
fn equip_sound(game: &mut Game<'_>, uuid: u128, stack: &ItemStack) {
    let sound = game
        .items()
        .get(&stack.item)
        .and_then(|i| i.equip_sound.clone())
        .unwrap_or_else(|| "minecraft:item.armor.equip_generic".to_owned());
    let Some((at, dimension)) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| (g.position, g.dimension.clone())) else {
        return;
    };
    let Some(level) = game.native_level(&dimension) else {
        return;
    };
    // `SoundSource.PLAYERS`.
    game.native_level_play_sound(level, &sound, at, 7, 1.0, 1.0);
}

// ---- totems and mob effects from accessory slots ------------------------------------------

/// Whether a stack saves from death (`DataComponents.DEATH_PROTECTION`).
fn death_protection(stack: &ItemStack) -> bool {
    &*stack.item == "minecraft:totem_of_undying" || stack.data().is_some_and(|d| d.other("minecraft:death_protection").is_some())
}

/// The accessory slots in a player's saved data (`guest.saved`'s `NeoForgeData`)
/// hold a totem: `AccessoriesEventHandler`'s `checkTotemDeathProtection` saves them.
pub(super) fn totem_in_slots(saved: Option<&Tag>) -> bool {
    let Some(Tag::Compound(root)) = saved else {
        return false;
    };
    let Some(Tag::Compound(data)) = root.get(&jkey("NeoForgeData")) else {
        return false;
    };
    let Some(Tag::Compound(h)) = data.get(&jkey(HOLDER)) else {
        return false;
    };
    let Some(Tag::Compound(containers)) = h.get(&jkey("accessories_containers")) else {
        return false;
    };
    let mut ignored = Vec::new();
    containers.values().any(|t| match t {
        Tag::Compound(sc) => stacks_from(sc.get(&jkey("items")), 256, &mut ignored).iter().flatten().any(death_protection),
        _ => false,
    })
}

/// A player the totem in their accessory slots saved: at the tick it is used up.
pub(super) fn totem_saved(uuid: u128) {
    with(|s| s.totems.push(uuid));
}

/// `LivingEntity.checkTotemDeathProtection` with the accessory's totem: used
/// up, the player alive at 1 health, their effects gone, then Regeneration II
/// (45 s), Absorption II (5 s) and Fire Resistance (40 s), its sound.
fn use_totem(game: &mut Game<'_>, uuid: u128) {
    let Some((mut holder, _)) = read_holder(game, uuid) else {
        return;
    };
    let mut used = false;
    for c in holder.values_mut() {
        if let Some(place) = c.items.iter_mut().find(|s| s.as_ref().is_some_and(death_protection)) {
            if let Some(s) = place.as_mut() {
                s.count -= 1;
                if s.count == 0 {
                    *place = None;
                }
            }
            used = true;
            break;
        }
    }
    if !used {
        return;
    }
    write_holder(game, uuid, &holder);
    with(|s| s.told.remove(&uuid));
    as_player(game, uuid, |g| {
        if let Some(p) = g.player.as_mut() {
            p.health.dead = false;
            let _ = p.set_health(1.0, false);
        }
    });
    game.native_set_effects(uuid, &[]);
    let effect = |id: &str, amplifier: u8, duration: i32| super::native_player_state::NativeEffect {
        id: id.to_owned(),
        amplifier,
        duration,
        ambient: false,
        particles: true,
    };
    game.native_add_effect(uuid, &effect("minecraft:regeneration", 1, 900));
    game.native_add_effect(uuid, &effect("minecraft:absorption", 1, 100));
    game.native_add_effect(uuid, &effect("minecraft:fire_resistance", 0, 800));
    if let Some((at, dimension)) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| (g.position, g.dimension.clone()))
        && let Some(level) = game.native_level(&dimension)
    {
        game.native_level_play_sound(level, "minecraft:item.totem.use", at, 7, 1.0, 1.0);
    }
}

/// One `MobEffectInstance` of an `accessories:mob_effects` component.
fn effect_of(tag: &Tag) -> Option<super::native_player_state::NativeEffect> {
    let Tag::Compound(c) = tag else {
        return None;
    };
    let id = match c.get(&jkey("id"))? {
        Tag::String(t) => s(t),
        _ => return None,
    };
    let int = |k: &str, d: i32| match c.get(&jkey(k)) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Byte(v)) => i32::from(*v),
        Some(Tag::Short(v)) => i32::from(*v),
        _ => d,
    };
    Some(super::native_player_state::NativeEffect {
        id: if id.contains(':') { id } else { format!("minecraft:{id}") },
        amplifier: int("amplifier", 0).clamp(0, 255) as u8,
        duration: int("duration", -1),
        ambient: int("ambient", 0) != 0,
        particles: int("show_particles", 1) != 0,
    })
}

/// A worn stack's `accessories:mob_effects`: its constant effects, and its
/// delayed ones with their delay.
fn mob_effects(stack: &ItemStack, items: &horizon_client::items::Items) -> (Vec<super::native_player_state::NativeEffect>, Vec<(i64, Vec<super::native_player_state::NativeEffect>)>) {
    let Some(Tag::Compound(c)) = menu::component(stack, items, "accessories:mob_effects") else {
        return (Vec::new(), Vec::new());
    };
    let constant = match c.get(&jkey("constant_effects")) {
        Some(Tag::List(l)) => l.iter().filter_map(effect_of).collect(),
        _ => Vec::new(),
    };
    let delayed = match c.get(&jkey("delayed_effects")) {
        Some(Tag::List(l)) => l
            .iter()
            .filter_map(|t| {
                let Tag::Compound(d) = t else {
                    return None;
                };
                let delay = match d.get(&jkey("delay")) {
                    Some(Tag::Int(v)) => i64::from(*v),
                    _ => return None,
                };
                let effects = match d.get(&jkey("effect_instances")) {
                    Some(Tag::List(l)) => l.iter().filter_map(effect_of).collect(),
                    _ => Vec::new(),
                };
                Some((delay.max(1), effects))
            })
            .collect(),
        _ => Vec::new(),
    };
    (constant, delayed)
}

thread_local! {
    /// The constant effects put on each player by their accessories, to take off.
    static EFFECTS: RefCell<HashMap<u128, Vec<String>>> = RefCell::new(HashMap::new());
}

/// `MobEffectsComponent`: the constant effects of worn accessories stay on
/// (taken off with them), the delayed ones come again every `delay` ticks.
fn apply_mob_effects(game: &mut Game<'_>) {
    let now = game.world.game_time;
    let players: Vec<u128> = game
        .lan
        .as_ref()
        .map(|h| h.hub.guests.iter().filter(|g| g.placed && !g.dead).map(|g| g.uuid).collect())
        .unwrap_or_default();
    for uuid in players {
        let Some((holder, _)) = read_holder(game, uuid) else {
            continue;
        };
        let items = game.items();
        let mut constant = Vec::new();
        let mut delayed = Vec::new();
        for (name, c) in &holder {
            if name.contains(':') {
                continue;
            }
            for stack in c.items.iter().flatten() {
                let mut stacks = vec![stack.clone()];
                stacks.extend(nested(stack, items));
                for s in stacks {
                    let (k, d) = mob_effects(&s, items);
                    constant.extend(k);
                    delayed.extend(d);
                }
            }
        }
        let ids: Vec<String> = constant.iter().map(|e| e.id.clone()).collect();
        let had = EFFECTS.with(|e| e.borrow().get(&uuid).cloned()).unwrap_or_default();
        if !had.is_empty() && had.iter().any(|h| !ids.contains(h)) {
            // Taken off: what no accessory gives any more goes.
            let left: Vec<_> = game.native_effects(uuid).unwrap_or_default().into_iter().filter(|e| !had.contains(&full_id(&e.id)) || ids.contains(&full_id(&e.id))).collect();
            game.native_set_effects(uuid, &left);
        }
        let active: Vec<String> = game.native_effects(uuid).unwrap_or_default().into_iter().map(|e| full_id(&e.id)).collect();
        for e in &constant {
            if !active.contains(&e.id) {
                game.native_add_effect(uuid, &super::native_player_state::NativeEffect { duration: -1, ..e.clone() });
            }
        }
        for (delay, effects) in &delayed {
            if now % delay == 0 {
                for e in effects {
                    game.native_add_effect(uuid, e);
                }
            }
        }
        EFFECTS.with(|e| e.borrow_mut().insert(uuid, ids));
    }
}

// ---- `/accessories` (`AccessoriesCommands`, `AccessoriesItemCommands`) ------------------

/// The commands, level 2. Ids hold a colon, which a client's word argument
/// refuses: what follows the command's words is one text, read here.
pub(super) fn register_commands(commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    use horizon_client::mod_api::NativeCommand;
    for usage in [
        "accessories feature <args:text>",
        "accessories edit",
        "accessories edit <target:player>",
        "accessories dump <args:text>",
        "accessories slot get <args:text>",
        "accessories slot modifier <args:text>",
        "accessories enchant <args:text>",
        "accessories components slot <args:text>",
        "accessories components stack-sizing <args:text>",
        "accessories components attribute <args:text>",
        "accessories components nest <item:item>",
        "accessories item replace <args:text>",
    ] {
        commands.push(NativeCommand::op(usage, 2));
    }
}

thread_local! {
    /// Commands to run at the next tick (the player's state is swapped in while one is heard).
    static PENDING_COMMANDS: RefCell<Vec<(u128, String)>> = const { RefCell::new(Vec::new()) };
}

/// `/accessories ...` from an operator: run at the tick.
pub(super) fn perform_command(player: &horizon_client::mod_api::NativeServerPlayer, command: &str, ran: &mut bool) {
    if command.split_whitespace().next() != Some("accessories") || player.permission < 2 {
        return;
    }
    *ran = true;
    PENDING_COMMANDS.with(|p| p.borrow_mut().push((player.uuid, command.to_owned())));
}

/// A player the command names (`@s`, `@p`, a name), else none.
fn command_player(game: &Game<'_>, word: Option<&str>, own: u128) -> Option<(u128, String)> {
    let host = game.lan.as_ref()?;
    match word {
        None | Some("@s" | "@p") => host.hub.guests.iter().find(|g| g.uuid == own).map(|g| (g.uuid, g.name.clone())),
        Some(name) => host.hub.guests.iter().find(|g| g.placed && g.name.eq_ignore_ascii_case(name)).map(|g| (g.uuid, g.name.clone())),
    }
}

/// The main hand's stack of player `uuid`, changed by `f` (its answer), written back.
fn edit_main_hand<T>(game: &mut Game<'_>, uuid: u128, f: impl FnOnce(&mut ItemStack) -> T) -> Option<T> {
    let g = game.lan.as_mut()?.hub.guests.iter_mut().find(|g| g.uuid == uuid)?;
    let stack = g.inventory.get_mut(g.selected)?.as_mut()?;
    Some(f(stack))
}

/// A stack's Accessories component as a compound to change.
fn component_compound(stack: &ItemStack, id: &str) -> Compound {
    match stack.data().and_then(|d| d.other(id)) {
        Some(Tag::Compound(c)) => c.clone(),
        _ => Compound::new(),
    }
}

fn set_component(stack: &mut ItemStack, id: &str, value: Compound) {
    let data = stack.data_mut();
    data.set_other(&format!("!{id}"), None);
    data.set_other(id, Some(Tag::Compound(value)));
    data.wire = Default::default();
    stack.tidy();
}

fn list_strings(c: &Compound, key: &str) -> Vec<String> {
    match c.get(&jkey(key)) {
        Some(Tag::List(l)) => l.iter().filter_map(|t| if let Tag::String(v) = t { Some(s(v)) } else { None }).collect(),
        _ => Vec::new(),
    }
}

fn set_strings(c: &mut Compound, key: &str, list: &[String]) {
    c.insert(jkey(key), Tag::List(list.iter().map(|v| Tag::String(jkey(v))).collect()));
}

/// A namespaced id: `foo` is `minecraft:foo`.
fn full_id(id: &str) -> String {
    if id.contains(':') { id.to_owned() } else { format!("minecraft:{id}") }
}

/// Runs the commands waiting, their answers in their player's chat.
fn run_commands(game: &mut Game<'_>) {
    for (own, command) in PENDING_COMMANDS.with(|p| std::mem::take(&mut *p.borrow_mut())) {
        run_command(game, own, &command);
    }
}

fn run_command(game: &mut Game<'_>, own: u128, command: &str) {
    let words: Vec<&str> = command.split_whitespace().collect();
    let say = |game: &mut Game<'_>, text: String| {
        game.native_tell(own, Text::literal(text));
    };
    let fail = |game: &mut Game<'_>, text: String| {
        game.native_tell(own, Text::literal(format!("§c{text}")));
    };
    let tr_s = |key: &str, args: &[String]| format_args(&horizon_client::lang::get_or(key, key), args);
    match &words[1..] {
        // `feature (enable|disable) (banner|glider|totem)`.
        ["feature", action @ ("enable" | "disable"), feature] => {
            let on = *action == "enable";
            let known = match *feature {
                "banner" => {
                    change_config(|c| c.content.allow_banner_equip = on);
                    true
                }
                "glider" => {
                    change_config(|c| c.content.allow_glider_equip = on);
                    true
                }
                "totem" => {
                    change_config(|c| c.content.allow_totem_equip = on);
                    true
                }
                _ => false,
            };
            if known {
                with(|s| s.slots_dirty = true);
                let key = if on { "accessories.commands.feature.toggle.on" } else { "accessories.commands.feature.toggle.off" };
                say(game, tr_s(key, &[(*feature).to_owned()]));
            } else {
                fail(game, tr_s("accessories.commands.feature.toggle.failure", &[(*feature).to_owned(), "banner, glider, totem".into()]));
            }
        }
        // `edit [player]`: the accessories screen of the player named, for the one running it.
        ["edit"] => open_for(game, own, own),
        ["edit", target] => match command_player(game, Some(target), own) {
            Some((uuid, _)) => open_for(game, own, uuid),
            None => fail(game, format!("No player was found: {target}")),
        },
        // `dump (slots|groups|entity_bindings)`: the data into the log.
        ["dump", kind] => {
            let text = with_data(|d| match *kind {
                "slots" => Some(format!("{:#?}", d.slots)),
                "groups" => Some(format!("{:#?}", d.groups)),
                "entity_bindings" => Some(format!("minecraft:player: {:?}", d.player)),
                _ => None,
            });
            match text {
                Some(text) => {
                    log::info!("[Accessories] {kind}:\n{text}");
                    say(game, tr_s("accessories.commands.dump.success", &[(*kind).to_owned()]));
                }
                None => fail(game, tr_s("accessories.commands.dump.failure", &[(*kind).to_owned(), "slots, groups, entity_bindings".into()])),
            }
        }
        // `slot get <player> <slot> [scale]`.
        ["slot", "get", target, slot, rest @ ..] => {
            let Some((uuid, name)) = command_player(game, Some(target), own) else {
                return fail(game, format!("No player was found: {target}"));
            };
            let scale: f64 = rest.first().and_then(|v| v.parse().ok()).unwrap_or(1.0);
            let size = read_holder(game, uuid).and_then(|(h, _)| h.get(*slot).map(|c| c.items.len())).unwrap_or(0);
            say(game, tr_s("accessories.commands.slot.value.get.success", &[(*slot).to_owned(), name, modifier_number(size as f64 * scale)]));
        }
        // `slot modifier clear <player> [slot]`.
        ["slot", "modifier", "clear", target, rest @ ..] => {
            let Some((uuid, name)) = command_player(game, Some(target), own) else {
                return fail(game, format!("No player was found: {target}"));
            };
            let Some((mut holder, _)) = read_holder(game, uuid) else {
                return;
            };
            match rest.first() {
                Some(slot) => {
                    if let Some(c) = holder.get_mut(*slot) {
                        c.modifiers.clear();
                    }
                    write_holder(game, uuid, &holder);
                    say(game, tr_s("accessories.commands.slot.modifier.clear.container.success", &[(*slot).to_owned(), name]));
                }
                None => {
                    for c in holder.values_mut() {
                        c.modifiers.clear();
                    }
                    write_holder(game, uuid, &holder);
                    say(game, tr_s("accessories.commands.slot.modifier.clear.all.success", &[name]));
                }
            }
            resize_after_change(game, uuid);
        }
        // `slot modifier <player> <slot> <id> (add <amount> <operation> [persistent] | remove | get [scale])`.
        ["slot", "modifier", target, slot, id, action, rest @ ..] => {
            let Some((uuid, name)) = command_player(game, Some(target), own) else {
                return fail(game, format!("No player was found: {target}"));
            };
            let Some((mut holder, _)) = read_holder(game, uuid) else {
                return;
            };
            let id = full_id(id);
            let Some(c) = holder.get_mut(*slot) else {
                return fail(game, format!("Unknown slot: {slot}"));
            };
            match (*action, rest) {
                ("add", [amount, op, ..]) => {
                    let Ok(amount) = amount.parse::<f64>() else {
                        return fail(game, format!("Not a number: {amount}"));
                    };
                    if !matches!(*op, "add_value" | "add_multiplied_base" | "add_multiplied_total") {
                        return fail(game, format!("Unknown operation: {op}"));
                    }
                    if c.modifiers.iter().any(|m| m.0 == id) {
                        return fail(game, tr_s("accessories.commands.attribute.failed.modifier_already_present_itemstack", &[id, (*slot).to_owned(), name]));
                    }
                    c.modifiers.push((id.clone(), amount, (*op).to_owned()));
                    write_holder(game, uuid, &holder);
                    resize_after_change(game, uuid);
                    say(game, tr_s("accessories.commands.slot.modifier.addition", &[id, (*slot).to_owned(), name]));
                }
                ("remove", _) => {
                    let before = c.modifiers.len();
                    c.modifiers.retain(|m| m.0 != id);
                    if c.modifiers.len() == before {
                        return fail(game, tr_s("accessories.commands.slot.modifier.removed.failure", &[id, (*slot).to_owned(), name]));
                    }
                    write_holder(game, uuid, &holder);
                    resize_after_change(game, uuid);
                    say(game, tr_s("accessories.commands.slot.modifier.removed.success", &[id, (*slot).to_owned(), name]));
                }
                ("get", rest) => {
                    let scale: f64 = rest.first().and_then(|v| v.parse().ok()).unwrap_or(1.0);
                    match c.modifiers.iter().find(|m| m.0 == id) {
                        Some(m) => say(game, tr_s("accessories.commands.attribute.modifier.value.get.success_itemstack", &[id.clone(), (*slot).to_owned(), name, modifier_number(m.1 * scale)])),
                        None => fail(game, tr_s("accessories.commands.slot.modifier.removed.failure", &[id, (*slot).to_owned(), name])),
                    }
                }
                _ => fail(game, "Unknown or incomplete command".into()),
            }
        }
        // `enchant <player> <enchantment> [level]`: as vanilla's, without its compatibility check.
        ["enchant", target, enchantment, rest @ ..] => {
            let Some((uuid, name)) = command_player(game, Some(target), own) else {
                return fail(game, format!("No player was found: {target}"));
            };
            let level: u32 = rest.first().and_then(|v| v.parse().ok()).unwrap_or(1);
            let id = full_id(enchantment);
            let done = edit_main_hand(game, uuid, |s| s.enchant(&id, level)).is_some();
            if done {
                say(game, format!("Applied enchantment {id} to {name}'s item"));
            } else {
                fail(game, format!("{name} isn't holding an item"));
            }
        }
        // `components slot (add|remove) (valid|invalid) <slot>`: the main hand's `accessories:slot_validation`.
        ["components", "slot", action @ ("add" | "remove"), kind @ ("valid" | "invalid"), slot] => {
            let key = format!("{kind}_slots");
            let add = *action == "add";
            let slot = (*slot).to_owned();
            let done = edit_main_hand(game, own, |s| {
                let mut c = component_compound(s, "accessories:slot_validation");
                let mut list = list_strings(&c, &key);
                list.retain(|v| *v != slot);
                if add {
                    list.push(slot.clone());
                }
                set_strings(&mut c, &key, &list);
                set_component(s, "accessories:slot_validation", c);
            });
            if done.is_some() {
                let k = format!("accessories.commands.slot.validation.{}.{kind}", if add { "added" } else { "removed" });
                say(game, tr_s(&k, &[slot]));
            }
        }
        // `components stack-sizing useStackSize <bool>` / `stack-sizing <size>`.
        ["components", "stack-sizing", "useStackSize", value] => {
            let on = *value == "true";
            edit_main_hand(game, own, |s| {
                let mut c = component_compound(s, "accessories:stack_settings");
                c.insert(jkey("use_stack_size"), Tag::Byte(i8::from(on)));
                set_component(s, "accessories:stack_settings", c);
            });
        }
        ["components", "stack-sizing", size] => {
            let size: i32 = size.parse().unwrap_or(1).max(1);
            edit_main_hand(game, own, |s| {
                let mut c = component_compound(s, "accessories:stack_settings");
                c.insert(jkey("size_override"), Tag::Int(size));
                set_component(s, "accessories:stack_settings", c);
            });
        }
        // `components attribute <attribute> <id> (add <amount> <operation> <slot> <stackable> [validation] | remove | get [scale])`.
        ["components", "attribute", attribute, id, action, rest @ ..] => {
            let attribute = full_id(attribute);
            let id = full_id(id);
            let item = edit_main_hand(game, own, |s| s.item.to_string()).unwrap_or_default();
            match (*action, rest) {
                ("add", [amount, op, slot, stackable, more @ ..]) => {
                    let amount: f64 = amount.parse().unwrap_or(0.0);
                    let validation = more.first() == Some(&"true");
                    let entry = |c: &mut Compound| {
                        c.insert(jkey("type"), Tag::String(jkey(&attribute)));
                        c.insert(jkey("id"), Tag::String(jkey(&id)));
                        c.insert(jkey("amount"), Tag::Double(amount.to_bits()));
                        c.insert(jkey("operation"), Tag::String(jkey(op)));
                        c.insert(jkey("slot_name"), Tag::String(jkey(slot)));
                        c.insert(jkey("is_stackable"), Tag::Byte(i8::from(*stackable == "true")));
                        c.insert(jkey("used_in_slot_validation"), Tag::Byte(i8::from(validation)));
                    };
                    let added = edit_main_hand(game, own, |s| {
                        let mut c = component_compound(s, "accessories:attributes");
                        let mut list = match c.get(&jkey("modifiers")) {
                            Some(Tag::List(l)) => l.clone(),
                            _ => Vec::new(),
                        };
                        let present = list.iter().any(|t| matches!(t, Tag::Compound(e) if e.get(&jkey("id")) == Some(&Tag::String(jkey(&id))) && e.get(&jkey("type")) == Some(&Tag::String(jkey(&attribute)))));
                        if present {
                            return false;
                        }
                        let mut e = Compound::new();
                        entry(&mut e);
                        list.push(Tag::Compound(e));
                        c.insert(jkey("modifiers"), Tag::List(list));
                        set_component(s, "accessories:attributes", c);
                        true
                    });
                    match added {
                        Some(true) => say(game, tr_s("accessories.commands.attribute.modifier.add.success_itemstack", &[id, attribute, item])),
                        Some(false) => fail(game, tr_s("accessories.commands.attribute.failed.modifier_already_present_itemstack", &[id, attribute, item])),
                        None => {}
                    }
                }
                ("remove", _) => {
                    let removed = edit_main_hand(game, own, |s| {
                        let mut c = component_compound(s, "accessories:attributes");
                        let mut list = match c.get(&jkey("modifiers")) {
                            Some(Tag::List(l)) => l.clone(),
                            _ => Vec::new(),
                        };
                        let before = list.len();
                        list.retain(|t| !matches!(t, Tag::Compound(e) if e.get(&jkey("id")) == Some(&Tag::String(jkey(&id))) && e.get(&jkey("type")) == Some(&Tag::String(jkey(&attribute)))));
                        let changed = list.len() != before;
                        c.insert(jkey("modifiers"), Tag::List(list));
                        set_component(s, "accessories:attributes", c);
                        changed
                    });
                    match removed {
                        Some(true) => say(game, tr_s("accessories.commands.attribute.modifier.remove.success_itemstack", &[id, attribute, item])),
                        Some(false) => fail(game, tr_s("accessories.commands.attribute.failed.no_modifier_itemstack", &[attribute, item, id])),
                        None => {}
                    }
                }
                ("get", rest) => {
                    let scale: f64 = rest.first().and_then(|v| v.parse().ok()).unwrap_or(1.0);
                    let items = game.items();
                    let found = game
                        .lan
                        .as_ref()
                        .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == own))
                        .and_then(|g| g.inventory.get(g.selected).cloned().flatten())
                        .and_then(|s| menu::modifiers(&s, items).0.into_iter().find(|m| m.id == id && m.attribute == attribute));
                    match found {
                        Some(m) => say(game, tr_s("accessories.commands.attribute.modifier.value.get.success_itemstack", &[id, attribute, item, modifier_number(m.amount * scale)])),
                        None => fail(game, tr_s("accessories.commands.attribute.failed.no_modifier_itemstack", &[attribute, item, id])),
                    }
                }
                _ => fail(game, "Unknown or incomplete command".into()),
            }
        }
        // `components nest <item>`: the item goes into the main hand's nest.
        ["components", "nest", item, ..] => {
            let inner = ItemStack::new(&full_id(item.split('[').next().unwrap_or(item)), 1);
            let done = edit_main_hand(game, own, |s| {
                let mut c = component_compound(s, "accessories:nested_accessories");
                let mut list = match c.get(&jkey("accessories")) {
                    Some(Tag::List(l)) => l.clone(),
                    _ => Vec::new(),
                };
                list.push(horizon_items::item_nbt::item_to_nbt(&inner));
                c.insert(jkey("accessories"), Tag::List(list));
                set_component(s, "accessories:nested_accessories", c);
            });
            if done.is_some() {
                say(game, tr_s("accessories.commands.nest.addition", &[]));
            }
        }
        // `item replace entity <player> <slot>/<index> with <item> [count]`.
        ["item", "replace", "entity", target, path, "with", item, rest @ ..] => {
            let Some((uuid, name)) = command_player(game, Some(target), own) else {
                return fail(game, format!("No player was found: {target}"));
            };
            let (slot, index) = match path.rsplit_once('/') {
                Some((s, i)) => (s.replace('-', ":"), i.parse::<usize>().unwrap_or(0)),
                None => ((*path).to_owned(), 0),
            };
            let count: u32 = rest.first().and_then(|v| v.parse().ok()).unwrap_or(1).max(1);
            let stack = ItemStack::new(&full_id(item.split('[').next().unwrap_or(item)), count);
            let Some((mut holder, _)) = read_holder(game, uuid) else {
                return;
            };
            match holder.get_mut(&slot).and_then(|c| c.items.get_mut(index)) {
                Some(place) => {
                    *place = Some(stack);
                    write_holder(game, uuid, &holder);
                    with(|s| s.told.remove(&uuid));
                    say(game, format!("Replaced a slot on {name} with {item}"));
                }
                None => fail(game, format!("No slot {path} on {name}")),
            }
        }
        _ => fail(game, "Unknown or incomplete command".into()),
    }
}

/// `/accessories edit`: player `owner`'s accessories screen for player `viewer`.
fn open_for(game: &mut Game<'_>, viewer: u128, owner: u128) {
    if let Some(open) = with(|s| s.open.get(&viewer).cloned())
        && let Some(host) = game.lan.as_mut()
        && let Some(conn) = host.hub.guests.iter().find(|g| g.uuid == viewer).map(|g| g.conn)
    {
        // The menu open closes first: its slots are written back at the next tick.
        menu::close(&mut host.hub, conn, open.window);
    }
    with(|s| s.opening.push((viewer, owner)));
}

/// A slot's size changed: what no longer fits goes back to the player.
fn resize_after_change(game: &mut Game<'_>, uuid: u128) {
    if let Some((holder, invalid)) = read_holder(game, uuid) {
        for s in invalid {
            give_back(game, uuid, s);
        }
        write_holder(game, uuid, &holder);
        with(|s| s.told.remove(&uuid));
    }
}

// ---- the client: the accessories screen (`AccessoriesScreen`) -----------------------------

/// What a menu slot is, as the screen draws it.
#[derive(Clone, Debug, PartialEq)]
enum Shown {
    Armor(usize),
    Offhand,
    /// A cosmetic armour slot (head first): its render toggle hides the armour.
    CosArmor { k: usize, render: bool },
    Slot { cosmetic: bool, slot: String, index: usize, group: String, icon: String, render: bool, used: bool },
    None,
}

/// The open accessories screen: what the server laid out, and what the player did since.
#[derive(Clone, Debug, Default)]
struct Screen {
    options: Options,
    /// Groups (name, icon sprite) in the order the filter lists them.
    groups: Vec<(String, String)>,
    slots: Vec<Shown>,
    /// The grid's first row shown (scrollable) or its page (paginated).
    scroll: usize,
    /// The settings panel is open in the inventory's place (`ADVANCED_SETTINGS`).
    settings: bool,
    /// The settings panel's first entry shown.
    settings_scroll: usize,
}

/// Where things are on the screen, from its top left.
#[derive(Clone, Debug, Default)]
struct Geo {
    width: f32,
    height: f32,
    /// The accessories grid's panel.
    grid: [f32; 4],
    rows: usize,
    total_rows: usize,
    /// Paginated: the pages (a title bar over the grid when more than one).
    pages: usize,
    /// Scrollable with more rows than shown: the scroll bar.
    scrolls: bool,
    /// The group filter's panel, if shown.
    side: Option<[f32; 4]>,
    entity: [f32; 2],
    entity_w: f32,
    /// The inventory's background.
    inventory: [f32; 2],
    /// The settings panel, in the inventory's place.
    settings: [f32; 4],
    /// Each menu slot's place, if shown.
    places: Vec<Option<[f32; 2]>>,
    /// The grid's slots shown, by menu slot, with their cell.
    cells: Vec<(usize, [f32; 2])>,
}

const ROWS_SHOWN: usize = 7;
const PAGE_ROWS: usize = 6;
const TOP_H: f32 = 140.0;
const INVENTORY_W: f32 = 198.0;
const INVENTORY_H: f32 = 90.0;
const SETTINGS_W: f32 = 200.0;
const ENTRY_H: f32 = 26.0;

/// The settings panel's entries (`AccessoriesScreen.createSettingsPanel`).
const SETTINGS: [&str; 12] = [
    "column_amount",
    "widget_type",
    "unused_slots",
    "side_by_side_slots",
    "side_by_side_entity",
    "main_widget_position",
    "group_filter",
    "side_widget_position",
    "entity_centered",
    "dark_mode_toggle",
    "show_equipped_stack_slot_type",
    "entity_look_at_cursor",
];

impl Screen {
    fn paginated(&self) -> bool {
        self.options.widget_type == 1
    }
    /// The column slider's range: 1 (3 paginated) to 9, 6 with the cosmetics
    /// beside, 4 with them and two entity views.
    fn column_range(&self) -> (i32, i32) {
        let min = if self.paginated() { 3 } else { 1 };
        let max = match (self.options.side_by_side_slots, self.options.side_by_side_entity) {
            (true, true) => 4,
            (true, false) => 6,
            _ => 9,
        };
        (min, max)
    }
    fn columns(&self) -> usize {
        let (min, max) = self.column_range();
        self.options.columns.clamp(min, max) as usize
    }
    /// Whether the cosmetics show beside the accessories (side by side, toggled on).
    fn cosmetics_beside(&self) -> bool {
        self.options.side_by_side_slots && self.options.cosmetic
    }
    /// The grid's slots: the accessories (or, switched, the cosmetics), used
    /// ones unless unused are shown, in the groups filtered to.
    fn grid_slots(&self) -> Vec<usize> {
        let o = &self.options;
        let want_cosmetic = !o.side_by_side_slots && o.cosmetic;
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| match s {
                Shown::Slot { cosmetic, group, used, .. } => {
                    *cosmetic == want_cosmetic && (*used || o.unused_slots) && (o.filtered_groups.is_empty() || o.filtered_groups.contains(group))
                }
                _ => false,
            })
            .map(|(i, _)| i)
            .collect()
    }
    /// The cosmetic slot of the same slot type and index as accessory slot `i`.
    fn partner(&self, i: usize) -> Option<usize> {
        let Some(Shown::Slot { slot, index, .. }) = self.slots.get(i) else {
            return None;
        };
        self.slots.iter().position(|s| matches!(s, Shown::Slot { cosmetic: true, slot: s2, index: i2, .. } if s2 == slot && i2 == index))
    }
    /// The groups the filter shows: those with a slot the grid could show.
    fn filter_groups(&self) -> Vec<(String, String)> {
        self.groups
            .iter()
            .filter(|(g, _)| {
                self.slots.iter().any(|s| matches!(s, Shown::Slot { group, used, cosmetic, .. } if group == g && (*used || self.options.unused_slots) && !*cosmetic))
            })
            .cloned()
            .collect()
    }
    fn geo(&self, dark: bool) -> Geo {
        let o = &self.options;
        let grid = self.grid_slots();
        let columns = self.columns();
        let total_rows = grid.len().div_ceil(columns).max(1);
        let paginated = self.paginated();
        let (rows, pages) = if paginated && total_rows > ROWS_SHOWN {
            (PAGE_ROWS, total_rows.div_ceil(PAGE_ROWS))
        } else {
            (total_rows.min(ROWS_SHOWN), 1)
        };
        let scrolls = !paginated && total_rows > ROWS_SHOWN;
        let title = if pages > 1 { 14.0 } else { 0.0 };
        let beside = self.cosmetics_beside();
        let cells_w = columns as f32 * 18.0;
        let grid_w = if beside { cells_w * 2.0 + 3.0 } else { cells_w } + 14.0 + if scrolls { 10.0 } else { 0.0 };
        let grid_h = rows as f32 * 18.0 + 14.0 + title;
        let groups = self.filter_groups();
        let side = (o.group_filter && !groups.is_empty()).then(|| {
            let h = 12.0 + 14.0 * (groups.len() + 1) as f32 + 2.0 * groups.len() as f32;
            [26.0, h.min(TOP_H)]
        });
        let entity_w = if o.side_by_side_entity { if dark { 176.0 } else { 178.0 } } else { 122.0 };
        // The top row: grid, group filter and entity, in the options' order.
        #[derive(Clone, Copy, PartialEq)]
        enum Part {
            Grid,
            Side,
            Entity,
        }
        let mut order = match (o.main_left, o.side_together) {
            (true, false) => vec![Part::Grid, Part::Entity, Part::Side],
            (true, true) => vec![Part::Grid, Part::Side, Part::Entity],
            (false, false) => vec![Part::Side, Part::Entity, Part::Grid],
            (false, true) => vec![Part::Entity, Part::Side, Part::Grid],
        };
        if side.is_none() {
            order.retain(|p| *p != Part::Side);
        }
        let w = |p: Part| match p {
            Part::Grid => grid_w,
            Part::Side => side.map_or(0.0, |s| s[0]),
            Part::Entity => entity_w,
        };
        let e = order.iter().position(|p| *p == Part::Entity).unwrap_or(0);
        let left: f32 = order[..e].iter().map(|p| w(*p) + 2.0).sum();
        let right: f32 = order[e + 1..].iter().map(|p| w(*p) + 2.0).sum();
        let row_w = if o.entity_centered { left.max(right) * 2.0 + entity_w } else { left + entity_w + right };
        let width = row_w.max(INVENTORY_W + 22.0).max(SETTINGS_W + 4.0);
        let ex = if o.entity_centered { (width - entity_w) / 2.0 } else { (width - row_w) / 2.0 + left };
        let mut xs = [0.0f32; 3];
        let mut x = ex;
        for p in order[..e].iter().rev() {
            x -= w(*p) + 2.0;
            xs[*p as usize] = x;
        }
        xs[Part::Entity as usize] = ex;
        let mut x = ex + entity_w + 2.0;
        for p in &order[e + 1..] {
            xs[*p as usize] = x;
            x += w(*p) + 2.0;
        }
        let gx = xs[Part::Grid as usize];
        let gy = (TOP_H - grid_h) / 2.0;
        let side = side.map(|[sw, sh]| [xs[Part::Side as usize], (TOP_H - sh) / 2.0, sw, sh]);
        let height = TOP_H + 2.0 + INVENTORY_H;
        let comp_x = (width - 176.0) / 2.0;
        let inventory = [comp_x - 22.0, TOP_H + 2.0];
        let settings = [(width - SETTINGS_W) / 2.0, TOP_H + 2.0, SETTINGS_W, INVENTORY_H];
        let first = if paginated { self.scroll.min(pages - 1) * PAGE_ROWS } else { self.scroll.min(total_rows.saturating_sub(rows)) };
        let mut places = vec![None; self.slots.len()];
        let mut cells = Vec::new();
        let grid_x = gx + 7.0 + if beside { cells_w + 3.0 } else { 0.0 };
        for (v, &i) in grid.iter().enumerate() {
            let (row, col) = (v / columns, v % columns);
            if row < first || row >= first + rows {
                continue;
            }
            let y = gy + 7.0 + title + (row - first) as f32 * 18.0;
            let cell = [grid_x + col as f32 * 18.0, y];
            places[i] = Some([cell[0] + 1.0, cell[1] + 1.0]);
            cells.push((i, cell));
            // `SIDE_BY_SIDE_SLOTS`: its cosmetic slot in the grid to the left.
            if beside && let Some(c) = self.partner(i) {
                let cell = [gx + 7.0 + col as f32 * 18.0, y];
                places[c] = Some([cell[0] + 1.0, cell[1] + 1.0]);
                cells.push((c, cell));
            }
        }
        let right_col = if o.side_by_side_entity { if dark { 152.0 } else { 154.0 } } else { 98.0 };
        for (i, s) in self.slots.iter().enumerate() {
            match s {
                Shown::Armor(k) => places[i] = Some([ex + 8.0, 31.0 + 18.0 * *k as f32]),
                Shown::CosArmor { k, .. } => places[i] = Some([ex + right_col, 31.0 + 18.0 * *k as f32]),
                Shown::Offhand if !self.settings => places[i] = Some([inventory[0] + 8.0, inventory[1] + 66.0]),
                _ => {}
            }
        }
        Geo {
            width,
            height,
            grid: [gx, gy, grid_w, grid_h],
            rows,
            total_rows,
            pages,
            scrolls,
            side,
            entity: [ex, 0.0],
            entity_w,
            inventory,
            settings,
            places,
            cells,
        }
    }
}

/// What the player asked on the screen, done at the client tick (the hooks
/// that hear it don't have the game to change).
#[derive(Clone, Debug, PartialEq)]
enum Ask {
    Toggle(usize),
    Option(String, String),
    Relayout,
    Back,
}

thread_local! {
    static SCREEN: RefCell<Option<Screen>> = const { RefCell::new(None) };
    static ASKS: RefCell<Vec<Ask>> = const { RefCell::new(Vec::new()) };
    /// The inventory's button was clicked: the screen opens at the tick.
    static OPEN_ASKED: Cell<bool> = const { Cell::new(false) };
    /// The equip control goes to the server (it told its slots: the player joined).
    static SEND_EQUIP_CONTROL: Cell<bool> = const { Cell::new(false) };
    /// The pointer is over a slot's render toggle: the item's tooltip gives way.
    static TOGGLE_HOVERED: Cell<bool> = const { Cell::new(false) };
    /// The accessory slot the pointer is over, holding an item (`Equipped in`).
    static HOVERED_SLOT: RefCell<Option<String>> = const { RefCell::new(None) };
    /// The accessories screen is the one shown (as last drawn).
    static TOOLTIP_SCREEN: Cell<bool> = const { Cell::new(false) };
    /// What each player wears, by profile name, as the server told.
    static WEARING: RefCell<HashMap<String, Vec<(String, usize, ItemStack)>>> = RefCell::new(HashMap::new());
}

fn ask(a: Ask) {
    ASKS.with(|q| q.borrow_mut().push(a));
}

fn with_screen(f: impl FnOnce(&mut Screen)) {
    SCREEN.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            f(s);
        }
    });
}

/// A player option changed here: kept, sent to the server, the screen laid out again.
fn set_option(name: &str, value: String) {
    with_screen(|s| {
        s.options.set(name, &value);
        s.scroll = 0;
    });
    ask(Ask::Option(name.to_owned(), value));
    ask(Ask::Relayout);
}

/// The layout the server sent ahead of the menu (`horizon:menu_extra`).
pub(super) fn screen_extra(menu: &str, extra: Option<&[u8]>) {
    if menu != MENU {
        return;
    }
    let Some(text) = extra.and_then(|e| std::str::from_utf8(e).ok()) else {
        return;
    };
    let mut screen = Screen::default();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        match parts.first().copied() {
            Some("opt") => screen.options = Options::decode(&parts[1..].join("\t")),
            Some("group") if parts.len() >= 3 => screen.groups.push((parts[1].to_owned(), parts[2].to_owned())),
            Some("slot") => {
                let shown = match parts.get(1).copied() {
                    Some("armor") => Shown::Armor(parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(0).min(3)),
                    Some("offhand") => Shown::Offhand,
                    Some("cosarmor") => {
                        let k = parts.get(2).and_then(|n| COSMETIC_ARMOR.iter().position(|p| n.ends_with(&format!(":{p}"))));
                        match k {
                            Some(k) if k < 4 => Shown::CosArmor { k, render: parts.get(3) != Some(&"0") },
                            _ => Shown::None,
                        }
                    }
                    Some(kind @ ("cosmetic" | "accessory")) if parts.len() >= 8 => Shown::Slot {
                        cosmetic: kind == "cosmetic",
                        slot: parts[2].to_owned(),
                        index: parts[3].parse().unwrap_or(0),
                        group: parts[4].to_owned(),
                        icon: parts[5].to_owned(),
                        render: parts[6] != "0",
                        used: parts[7] != "0",
                    },
                    _ => Shown::None,
                };
                screen.slots.push(shown);
            }
            _ => {}
        }
    }
    SCREEN.with(|s| *s.borrow_mut() = Some(screen));
}

/// The screen's size and where each slot is (`native_screen`); the
/// inventory goes away while the settings panel is in its place.
pub(super) fn screen(menu: &str, out: &mut Option<NativeScreen>) {
    if menu != MENU {
        return;
    }
    let Some(screen) = SCREEN.with(|s| s.borrow().clone()) else {
        return;
    };
    let geo = screen.geo(config().is_dark_mode);
    let mut slots: Vec<Option<[f32; 2]>> = geo.places.clone();
    slots.resize(256, None);
    let hidden = if screen.settings { -10000.0 } else { 0.0 };
    *out = Some(NativeScreen {
        width: geo.width,
        height: geo.height,
        slots,
        inventory_y: geo.inventory[1] + 8.0 + hidden,
        inventory_x: geo.inventory[0] + 30.0,
        own_labels: true,
        own_background: true,
    });
}

fn theme(dark: bool) -> &'static str {
    if dark { "dark" } else { "light" }
}

fn inside([x, y, w, h]: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= x && at[0] < x + w && at[1] >= y && at[1] < y + h
}

/// A nine-patch (`NinePatchTexture`): part `[u, v, w, h]` of sprite `name`,
/// its corners `corner` pixels, stretched over `dst`.
fn nine(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, name: &str, [u, v, w, h]: [f32; 4], corner: f32, [x, y, dw, dh]: [f32; 4]) {
    let c = corner;
    let (mw, mh) = (w - 2.0 * c, h - 2.0 * c);
    let (dmw, dmh) = ((dw - 2.0 * c).max(0.0), (dh - 2.0 * c).max(0.0));
    let cols = [(u, c, x, c), (u + c, mw, x + c, dmw), (u + w - c, c, x + dw - c, c)];
    let rows = [(v, c, y, c), (v + c, mh, y + c, dmh), (v + h - c, c, y + dh - c, c)];
    for &(su, sw, dx, dwid) in &cols {
        for &(sv, sh, dy, dhei) in &rows {
            if dwid > 0.0 && dhei > 0.0 {
                game.native_sprite(batch, name, [su, sv, sw, sh], [dx, dy, dwid, dhei]);
            }
        }
    }
}

fn panel(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, dark: bool, rect: [f32; 4]) {
    nine(game, batch, &format!("accessories:gui/theme/{}/panel", theme(dark)), [0.0, 0.0, 16.0, 16.0], 5.0, rect);
}

fn inset(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, dark: bool, rect: [f32; 4]) {
    nine(game, batch, &format!("accessories:gui/theme/{}/inset", theme(dark)), [0.0, 0.0, 16.0, 16.0], 5.0, rect);
}

/// A theme button (`theme/<mode>/button/(active|hovered|disabled)`).
fn button(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, dark: bool, rect: [f32; 4], hovered: bool) {
    let v = if hovered { 64.0 } else { 0.0 };
    nine(game, batch, &format!("accessories:gui/theme/{}/button", theme(dark)), [0.0, v, 64.0, 64.0], 3.0, rect);
}

/// A 14x14 (or 10x10) icon button, its hovered look under the pointer.
fn icon_button(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, icon: &str, size: f32, [x, y]: [f32; 2], mouse: [f32; 2]) -> bool {
    let over = inside([x, y, size, size], mouse);
    let name = if over { format!("accessories:gui/{icon}_hovered") } else { format!("accessories:gui/{icon}") };
    game.native_sprite(batch, &name, [0.0, 0.0, size, size], [x, y, size, size]);
    over
}

/// The entity panel's corner buttons (`cosmetic`, `back`, `settings`), from the panel's top left.
fn corner_buttons(geo: &Geo) -> [[f32; 4]; 3] {
    let [ex, ey] = geo.entity;
    let w = geo.entity_w;
    [[ex + 9.0, ey + 9.0, 14.0, 14.0], [ex + w - 19.0, ey + 9.0, 10.0, 10.0], [ex + w - 23.0, ey + TOP_H - 23.0, 14.0, 14.0]]
}

/// The paginated grid's page buttons (previous, next) in its title bar.
fn page_buttons(geo: &Geo) -> [[f32; 4]; 2] {
    let [gx, gy, gw, _] = geo.grid;
    [[gx + 6.0, gy + 6.0, 10.0, 10.0], [gx + gw - 16.0, gy + 6.0, 10.0, 10.0]]
}

/// A slot's 5x5 render toggle, at its cell's top right (`ToggleButton`).
fn toggle_rect(cell: [f32; 2]) -> [f32; 4] {
    [cell[0] + 14.0, cell[1] - 1.0, 5.0, 5.0]
}

/// The occupied menu slots, from the data slots (16 a word).
fn occupied(data: &[i32], i: usize) -> bool {
    data.get(i / 16).is_some_and(|w| (*w >> (i % 16)) & 1 == 1)
}

/// The settings panel's entry `k`: its label's and button's places.
fn entry_rects(geo: &Geo, scroll: usize, k: usize) -> Option<([f32; 2], [f32; 4])> {
    let [x, y, w, h] = geo.settings;
    let row = k.checked_sub(scroll)? as f32;
    let top = y + 5.0 + row * ENTRY_H;
    if top + ENTRY_H > y + h - 2.0 {
        return None;
    }
    Some(([x + 8.0, top + 2.0], [x + (w - 148.0) / 2.0, top + 12.0, 148.0, 14.0]))
}

/// A settings entry's value as its button says it, and whether it is on.
fn entry_value(game: &Game<'_>, s: &Screen, name: &str) -> String {
    let o = &s.options;
    let c = config();
    let toggle = |on: bool| tr(game, &format!("accessories.{name}.toggle.{}", if on { "enabled" } else { "disabled" }));
    match name {
        "column_amount" => s.columns().to_string(),
        "widget_type" => tr(game, if o.widget_type == 1 { "accessories.widget_type.paginated" } else { "accessories.widget_type.scrollable" }),
        "unused_slots" => toggle(o.unused_slots),
        "side_by_side_slots" => toggle(o.side_by_side_slots),
        "side_by_side_entity" => toggle(o.side_by_side_entity),
        "main_widget_position" => toggle(o.main_left),
        "group_filter" => toggle(o.group_filter),
        "side_widget_position" => toggle(o.side_together),
        "entity_centered" => toggle(o.entity_centered),
        "dark_mode_toggle" => toggle(c.is_dark_mode),
        "show_equipped_stack_slot_type" => toggle(c.show_equipped_stack_slot_type),
        "entity_look_at_cursor" => toggle(c.entity_looks_at_mouse_cursor),
        _ => String::new(),
    }
}

/// Draws the accessories screen under its slots' items (`native_screen_draw`).
pub(super) fn screen_draw(game: &Game<'_>, menu: &str, data: &[i32], batch: &mut horizon_client::hud::HudBatch, origin: [f32; 2]) {
    if menu != MENU {
        return;
    }
    let Some(screen) = SCREEN.with(|s| s.borrow().clone()) else {
        return;
    };
    let dark = config().is_dark_mode;
    let geo = screen.geo(dark);
    let t = theme(dark);
    let [ox, oy] = origin;
    let cursor = game.native_cursor();
    let mouse = [cursor[0] - ox, cursor[1] - oy];
    let at = |[x, y]: [f32; 2]| [ox + x, oy + y];
    let text_colour = if dark { 0x00FF_FFFF } else { 0x0040_4040 };
    // The inventory's background with the off hand at its left, or the settings panel.
    if screen.settings {
        let [sx, sy, sw, sh] = geo.settings;
        inset(game, batch, dark, [ox + sx, oy + sy, sw, sh]);
        for (k, name) in SETTINGS.iter().enumerate() {
            let Some((label, b)) = entry_rects(&geo, screen.settings_scroll, k) else {
                continue;
            };
            game.native_text(batch, &tr(game, &format!("accessories.{name}.label")), at(label), text_colour, false);
            let over = inside(b, mouse);
            button(game, batch, dark, [ox + b[0], oy + b[1], b[2], b[3]], over);
            if *name == "column_amount" {
                // `ColumnSlider`: a knob along the button, the columns over it.
                let (min, max) = screen.column_range();
                let f = (screen.columns() as f32 - min as f32) / (max - min).max(1) as f32;
                let kx = b[0] + 1.0 + f * (b[2] - 10.0);
                button(game, batch, dark, [ox + kx, oy + b[1], 8.0, b[3]], true);
            }
            let value = entry_value(game, &screen, name);
            let tw = game.native_text_width(&value);
            game.native_text(batch, &value, [ox + b[0] + (b[2] - tw) / 2.0, oy + b[1] + 3.0], 0x00FF_FFFF, true);
        }
    } else {
        let [ix, iy] = geo.inventory;
        game.native_sprite(
            batch,
            &format!("accessories:gui/theme/{t}/player_inv/without_crafting"),
            [0.0, 0.0, INVENTORY_W, INVENTORY_H],
            [ox + ix, oy + iy, INVENTORY_W, INVENTORY_H],
        );
    }
    // The entity panel: background, the player (twice side by side), the frame with its armour columns.
    let [ex, ey] = geo.entity;
    let view = if screen.options.side_by_side_entity { "double" } else { "single" };
    let bg_w = if screen.options.side_by_side_entity { 162.0 } else { 108.0 };
    game.native_sprite(
        batch,
        &format!("accessories:gui/theme/{t}/entity_view/{view}/entity_background"),
        [0.0, 0.0, bg_w, 126.0],
        [ox + ex + 7.0, oy + ey + 7.0, bg_w, 126.0],
    );
    if screen.options.side_by_side_entity {
        let half = bg_w / 2.0;
        game.draw_player_preview(batch, ox + ex + 7.0, oy + ey + 7.0, ox + ex + 7.0 + half, oy + ey + 133.0, 45.0, 0.0625);
        game.draw_player_preview(batch, ox + ex + 7.0 + half, oy + ey + 7.0, ox + ex + 7.0 + bg_w, oy + ey + 133.0, 45.0, 0.0625);
    } else {
        game.draw_player_preview(batch, ox + ex + 7.0, oy + ey + 7.0, ox + ex + 115.0, oy + ey + 133.0, 50.0, 0.0625);
    }
    game.native_sprite(
        batch,
        &format!("accessories:gui/theme/{t}/entity_view/{view}/4_slots"),
        [0.0, 0.0, geo.entity_w, TOP_H],
        [ox + ex, oy + ey, geo.entity_w, TOP_H],
    );
    // The armour slots' empty icons, and the cosmetic armour's toggles.
    const ARMOR_ICONS: [&str; 4] = ["helmet", "chestplate", "leggings", "boots"];
    for (i, s) in screen.slots.iter().enumerate() {
        let Some(place) = geo.places.get(i).copied().flatten() else {
            continue;
        };
        match s {
            Shown::Armor(k) | Shown::CosArmor { k, .. } if !occupied(data, i) => {
                let name = format!("gui/sprites/container/slot/{}", ARMOR_ICONS[*k]);
                let [x, y] = at(place);
                game.native_sprite(batch, &name, [0.0, 0.0, 16.0, 16.0], [x, y, 16.0, 16.0]);
            }
            Shown::Offhand if !occupied(data, i) => {
                let [x, y] = at(place);
                game.native_sprite(batch, "gui/sprites/container/slot/shield", [0.0, 0.0, 16.0, 16.0], [x, y, 16.0, 16.0]);
            }
            _ => {}
        }
        if let Shown::CosArmor { render, .. } = s {
            draw_toggle(game, batch, dark, *render, at([place[0] - 1.0, place[1] - 1.0]), mouse, [place[0] - 1.0, place[1] - 1.0]);
        }
    }
    // Whether the pointer is over a render toggle (the item tooltip gives way).
    let mut over_toggle = geo.cells.iter().any(|&(i, cell)| toggles(&screen, i) && inside(toggle_rect(cell), mouse));
    for (i, s) in screen.slots.iter().enumerate() {
        if let (Shown::CosArmor { .. }, Some(p)) = (s, geo.places.get(i).copied().flatten()) {
            over_toggle |= inside(toggle_rect([p[0] - 1.0, p[1] - 1.0]), mouse);
        }
    }
    TOGGLE_HOVERED.with(|t| t.set(over_toggle));
    let hovered = geo.places.iter().enumerate().find_map(|(i, p)| {
        let [x, y] = (*p)?;
        match screen.slots.get(i) {
            Some(Shown::Slot { slot, .. }) if inside([x - 1.0, y - 1.0, 18.0, 18.0], mouse) && occupied(data, i) => Some(slot.clone()),
            _ => None,
        }
    });
    HOVERED_SLOT.with(|h| *h.borrow_mut() = hovered);
    // The accessories grid (`AccessoriesContainingComponent`).
    panel(game, batch, dark, [ox + geo.grid[0], oy + geo.grid[1], geo.grid[2], geo.grid[3]]);
    if geo.pages > 1 {
        // `PaginatedComponent`'s title bar: n/N between its arrows.
        let [prev, next] = page_buttons(&geo);
        let page = screen.scroll.min(geo.pages - 1);
        let label = format!("{}/{}", page + 1, geo.pages);
        let tw = game.native_text_width(&label);
        game.native_text(batch, &label, [ox + geo.grid[0] + (geo.grid[2] - tw) / 2.0, oy + geo.grid[1] + 7.0], text_colour, false);
        icon_button(game, batch, "accessories_back_icon", 10.0, at([prev[0], prev[1]]), cursor);
        let name = if inside(next, mouse) { "accessories:gui/accessories_back_icon_hovered" } else { "accessories:gui/accessories_back_icon" };
        // The next arrow is the back arrow turned around.
        game.native_sprite(batch, name, [10.0, 10.0, -10.0, -10.0], [ox + next[0], oy + next[1], 10.0, 10.0]);
    }
    let darken = config().show_slot_darkening_effect;
    for &(i, cell) in &geo.cells {
        let [cx, cy] = at(cell);
        if dark {
            game.native_sprite(batch, "accessories:gui/theme/dark/slot", [0.0, 0.0, 18.0, 18.0], [cx, cy, 18.0, 18.0]);
        } else {
            game.native_sprite(batch, "gui/sprites/container/slot", [0.0, 0.0, 18.0, 18.0], [cx, cy, 18.0, 18.0]);
        }
        if let Some(Shown::Slot { icon, render, slot, .. }) = screen.slots.get(i) {
            if !occupied(data, i) {
                game.native_sprite(batch, icon, [0.0, 0.0, 16.0, 16.0], [cx + 1.0, cy + 1.0, 16.0, 16.0]);
            }
            // `showSlotDarkeningEffect`: carrying a stack, the slots that won't take it darken.
            if darken
                && let Some(carried) = game.carried.as_ref()
                && !occupied(data, i)
                && !CLIENT_SLOTS.with(|c| c.borrow().iter().any(|(r, _)| r.slot == *slot && menu::valid(r, carried, game.items())))
            {
                game.native_fill(batch, [cx + 1.0, cy + 1.0, 16.0, 16.0], 0x4D00_0000);
            }
            if toggles(&screen, i) {
                draw_toggle(game, batch, dark, *render, [cx, cy], mouse, cell);
            }
        }
    }
    // The grid's scroll bar, when it has more rows than it shows.
    if geo.scrolls {
        let [gx, gy, gw, gh] = geo.grid;
        let track = [ox + gx + gw - 15.0, oy + gy + 7.0, 8.0, gh - 14.0];
        nine(game, batch, &format!("accessories:gui/theme/{t}/scrollbar"), [26.0, 0.0, 6.0, 32.0], 1.0, track);
        let max = (geo.total_rows - geo.rows) as f32;
        let knob_h = (track[3] * geo.rows as f32 / geo.total_rows as f32).max(8.0);
        let pos = screen.scroll.min(geo.total_rows - geo.rows) as f32 / max;
        nine(game, batch, &format!("accessories:gui/theme/{t}/scrollbar"), [0.0, 0.0, 12.0, 16.0], 2.0, [track[0], track[1] + (track[3] - knob_h) * pos, 8.0, knob_h]);
    }
    // The group filter (`GroupFilterComponent`).
    if let Some(side) = geo.side {
        panel(game, batch, dark, [ox + side[0], oy + side[1], side[2], side[3]]);
        let (bx, mut by) = (side[0] + 6.0, side[1] + 6.0);
        icon_button(game, batch, "reset_icon", 14.0, at([bx, by]), cursor);
        by += 16.0;
        for (name, icon) in screen.filter_groups() {
            let selected = screen.options.filtered_groups.contains(&name);
            let rect = [bx, by, 14.0, 14.0];
            let v = match (selected, inside(rect, mouse)) {
                (true, false) => 0.0,
                (true, true) => 64.0,
                (false, false) => 128.0,
                (false, true) => 192.0,
            };
            nine(game, batch, &format!("accessories:gui/theme/{t}/button_toggle_rounded"), [0.0, v, 64.0, 64.0], 3.0, [ox + bx, oy + by, 14.0, 14.0]);
            game.native_sprite(batch, &icon, [0.0, 0.0, 8.0, 8.0], [ox + bx + 3.0, oy + by + 3.0, 8.0, 8.0]);
            by += 16.0;
        }
    }
    // The entity panel's corner buttons.
    let [cosmetic, back, settings] = corner_buttons(&geo);
    let icon = if screen.options.cosmetic { "charm_toggle_icon" } else { "cosmetic_toggle_icon" };
    icon_button(game, batch, icon, 14.0, at([cosmetic[0], cosmetic[1]]), cursor);
    icon_button(game, batch, "accessories_back_icon", 10.0, at([back[0], back[1]]), cursor);
    icon_button(game, batch, "settings_icon", 14.0, at([settings[0], settings[1]]), cursor);
}

/// Whether grid slot `i` has a render toggle: the accessories' (not the
/// cosmetics' beside them), and the cosmetics' when switched to.
fn toggles(screen: &Screen, i: usize) -> bool {
    match screen.slots.get(i) {
        Some(Shown::Slot { cosmetic, .. }) => !*cosmetic || !screen.options.side_by_side_slots,
        _ => false,
    }
}

/// A slot's render toggle (`theme/<mode>/button/toggle/rounded`), at its cell's top right.
fn draw_toggle(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, dark: bool, on: bool, cell_at: [f32; 2], mouse: [f32; 2], cell: [f32; 2]) {
    let r = toggle_rect(cell);
    let over = inside(r, mouse);
    let v = match (on, over) {
        (true, false) => 0.0,
        (true, true) => 64.0,
        (false, false) => 128.0,
        (false, true) => 192.0,
    };
    let dst = [cell_at[0] + 14.0, cell_at[1] - 1.0, 5.0, 5.0];
    nine(game, batch, &format!("accessories:gui/theme/{}/button_toggle_rounded", theme(dark)), [0.0, v, 64.0, 64.0], 2.0, dst);
}

/// The open accessories screen's panel, when it is the one shown.
fn our_screen(screen: &horizon_client::mod_api::NativeScreenView<'_>) -> bool {
    screen.menu == Some(MENU)
}

/// A settings entry clicked at `x` along its button.
fn settings_click(s: &Screen, name: &str, b: [f32; 4], x: f32) {
    let o = &s.options;
    let flip = |v: bool| u8::from(!v).to_string();
    match name {
        "column_amount" => {
            let (min, max) = s.column_range();
            let f = ((x - b[0] - 5.0) / (b[2] - 10.0)).clamp(0.0, 1.0);
            let cols = min + (f * (max - min) as f32).round() as i32;
            set_option("column_amount", cols.to_string());
        }
        "widget_type" => set_option("widget_type", if o.widget_type == 1 { "2".into() } else { "1".into() }),
        "unused_slots" => set_option("unused_slots", flip(o.unused_slots)),
        "side_by_side_slots" => set_option("side_by_side_slots", flip(o.side_by_side_slots)),
        "side_by_side_entity" => set_option("side_by_side_entity", flip(o.side_by_side_entity)),
        "main_widget_position" => set_option("main_widget_position", flip(o.main_left)),
        "group_filter" => set_option("group_filter", flip(o.group_filter)),
        "side_widget_position" => set_option("side_widget_position", flip(o.side_together)),
        "entity_centered" => set_option("entity_centered", flip(o.entity_centered)),
        "dark_mode_toggle" => {
            change_config(|c| c.is_dark_mode = !c.is_dark_mode);
            ask(Ask::Relayout);
        }
        "show_equipped_stack_slot_type" => change_config(|c| c.show_equipped_stack_slot_type = !c.show_equipped_stack_slot_type),
        "entity_look_at_cursor" => change_config(|c| c.entity_looks_at_mouse_cursor = !c.entity_looks_at_mouse_cursor),
        _ => {}
    }
}

/// A press on the accessories screen: its buttons and toggles take it before the slots.
fn screen_press(screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if button != 0 {
        return;
    }
    let Some(s) = SCREEN.with(|s| s.borrow().clone()) else {
        return;
    };
    let geo = s.geo(config().is_dark_mode);
    let at = [screen.mouse[0] - screen.rect[0], screen.mouse[1] - screen.rect[1]];
    let [cosmetic, back, settings] = corner_buttons(&geo);
    if inside(cosmetic, at) {
        *canceled = true;
        set_option("cosmetic", u8::from(!s.options.cosmetic).to_string());
        return;
    }
    if inside(back, at) {
        *canceled = true;
        ask(Ask::Back);
        return;
    }
    if inside(settings, at) {
        *canceled = true;
        with_screen(|s| {
            s.settings = !s.settings;
            s.settings_scroll = 0;
        });
        ask(Ask::Relayout);
        return;
    }
    if s.settings {
        for (k, name) in SETTINGS.iter().enumerate() {
            if let Some((_, b)) = entry_rects(&geo, s.settings_scroll, k)
                && inside(b, at)
            {
                *canceled = true;
                settings_click(&s, name, b, at[0]);
                return;
            }
        }
    }
    if geo.pages > 1 {
        let [prev, next] = page_buttons(&geo);
        let page = s.scroll.min(geo.pages - 1);
        if inside(prev, at) || inside(next, at) {
            *canceled = true;
            let to = if inside(prev, at) { page.saturating_sub(1) } else { (page + 1).min(geo.pages - 1) };
            with_screen(|s| s.scroll = to);
            ask(Ask::Relayout);
            return;
        }
    }
    // Render toggles: the grid's and the cosmetic armour's.
    let mut toggled = None;
    for &(i, cell) in &geo.cells {
        if toggles(&s, i) && inside(toggle_rect(cell), at) {
            toggled = Some(i);
        }
    }
    for (i, shown) in s.slots.iter().enumerate() {
        if let (Shown::CosArmor { .. }, Some(place)) = (shown, geo.places.get(i).copied().flatten())
            && inside(toggle_rect([place[0] - 1.0, place[1] - 1.0]), at)
        {
            toggled = Some(i);
        }
    }
    if let Some(i) = toggled {
        *canceled = true;
        with_screen(|c| {
            if let Some(Shown::Slot { render, .. } | Shown::CosArmor { render, .. }) = c.slots.get_mut(i) {
                *render = !*render;
            }
        });
        ask(Ask::Toggle(i));
        return;
    }
    // The group filter: reset, or a group in or out.
    if let Some(side) = geo.side {
        let (bx, mut by) = (side[0] + 6.0, side[1] + 6.0);
        if inside([bx, by, 14.0, 14.0], at) {
            *canceled = true;
            set_option("filtered_groups", String::new());
            return;
        }
        by += 16.0;
        let groups = s.filter_groups();
        for (name, _) in &groups {
            if inside([bx, by, 14.0, 14.0], at) {
                *canceled = true;
                let mut chosen = s.options.filtered_groups.clone();
                match chosen.iter().position(|g| g == name) {
                    Some(k) => {
                        chosen.remove(k);
                    }
                    None => chosen.push(name.clone()),
                }
                // Every group chosen is none chosen.
                if chosen.len() >= groups.len() {
                    chosen.clear();
                }
                set_option("filtered_groups", chosen.join(","));
                return;
            }
            by += 16.0;
        }
    }
}

/// The wheel: the grid a row (or a page), the settings panel an entry.
pub(super) fn screen_scrolled(screen: &horizon_client::mod_api::NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    if !our_screen(screen) || scroll[1] == 0.0 {
        return;
    }
    let Some(s) = SCREEN.with(|s| s.borrow().clone()) else {
        return;
    };
    let geo = s.geo(config().is_dark_mode);
    let at = [screen.mouse[0] - screen.rect[0], screen.mouse[1] - screen.rect[1]];
    let up = scroll[1] > 0.0;
    if s.settings && inside(geo.settings, at) {
        *canceled = true;
        let shown = ((geo.settings[3] - 7.0) / ENTRY_H).floor() as usize;
        let max = SETTINGS.len().saturating_sub(shown.max(1));
        let next = if up { s.settings_scroll.saturating_sub(1) } else { (s.settings_scroll + 1).min(max) };
        with_screen(|s| s.settings_scroll = next);
        return;
    }
    if !inside(geo.grid, at) {
        return;
    }
    if geo.pages > 1 {
        // The wheel over the title bar's arrows turns the page.
        let [prev, next] = page_buttons(&geo);
        if inside([prev[0], prev[1], next[0] + next[2] - prev[0], 10.0], at) {
            *canceled = true;
            let page = s.scroll.min(geo.pages - 1);
            let to = if up { page.saturating_sub(1) } else { (page + 1).min(geo.pages - 1) };
            with_screen(|s| s.scroll = to);
            ask(Ask::Relayout);
        }
        return;
    }
    if !geo.scrolls {
        return;
    }
    if !config().allow_slot_scrolling {
        let [gx, gy, gw, gh] = geo.grid;
        if !inside([gx + gw - 15.0, gy + 7.0, 8.0, gh - 14.0], at) {
            return;
        }
    }
    *canceled = true;
    let max = geo.total_rows - geo.rows;
    let next = if up { s.scroll.min(max).saturating_sub(1) } else { (s.scroll + 1).min(max) };
    with_screen(|s| s.scroll = next);
    ask(Ask::Relayout);
}

/// The tooltip over the screen's own parts: a slot's name, the buttons', the settings'.
pub(super) fn screen_tooltip(game: &Game<'_>, menu: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if menu != MENU {
        return;
    }
    let Some(s) = SCREEN.with(|s| s.borrow().clone()) else {
        return;
    };
    let geo = s.geo(config().is_dark_mode);
    let [cosmetic, back, settings] = corner_buttons(&geo);
    if inside(cosmetic, at) {
        let k = if s.options.cosmetic { "accessories.slot_cosmetics.toggle.enabled" } else { "accessories.slot_cosmetics.toggle.disabled" };
        lines.push(tr(game, k));
        return;
    }
    if inside(back, at) {
        lines.push(tr(game, "accessories.back.screen"));
        return;
    }
    if inside(settings, at) {
        let k = if s.settings { "accessories.advanced_options.toggle.enabled.tooltip" } else { "accessories.advanced_options.toggle.disabled.tooltip" };
        lines.push(tr(game, k));
        return;
    }
    if s.settings {
        for (k, name) in SETTINGS.iter().enumerate() {
            if let Some((_, b)) = entry_rects(&geo, s.settings_scroll, k)
                && inside(b, at)
            {
                let o = &s.options;
                let c = config();
                let on = match *name {
                    "unused_slots" => o.unused_slots,
                    "side_by_side_slots" => o.side_by_side_slots,
                    "side_by_side_entity" => o.side_by_side_entity,
                    "main_widget_position" => o.main_left,
                    "group_filter" => o.group_filter,
                    "side_widget_position" => o.side_together,
                    "entity_centered" => o.entity_centered,
                    "dark_mode_toggle" => c.is_dark_mode,
                    "show_equipped_stack_slot_type" => c.show_equipped_stack_slot_type,
                    "entity_look_at_cursor" => c.entity_looks_at_mouse_cursor,
                    _ => false,
                };
                let key = match *name {
                    "column_amount" => "accessories.column_amount.tooltip".to_owned(),
                    "widget_type" => format!("accessories.widget_type.{}.tooltip", if o.widget_type == 1 { "paginated" } else { "scrollable" }),
                    _ => format!("accessories.{name}.toggle.{}.tooltip", if on { "enabled" } else { "disabled" }),
                };
                lines.push(tr(game, &key));
                return;
            }
        }
    }
    if let Some(side) = geo.side {
        let (bx, mut by) = (side[0] + 6.0, side[1] + 6.0);
        if inside([bx, by, 14.0, 14.0], at) {
            lines.push(tr(game, "accessories.reset.group_filter"));
            return;
        }
        by += 16.0;
        for (name, _) in s.filter_groups() {
            if inside([bx, by, 14.0, 14.0], at) {
                lines.push(tr(game, &format!("accessories.slot_group.{}", name.replace(':', "."))));
                return;
            }
            by += 16.0;
        }
    }
    for (i, place) in geo.places.iter().enumerate() {
        let Some([x, y]) = *place else {
            continue;
        };
        if !inside([x - 1.0, y - 1.0, 18.0, 18.0], at) {
            continue;
        }
        if let Some(Shown::Slot { slot, cosmetic, .. }) = s.slots.get(i) {
            let name = tr(game, &format!("accessories.slot.{}", slot.replace(':', ".")));
            // `AccessoriesBasedSlot.getTooltip`: `Slot: <name>` on an empty slot (an
            // item's own tooltip says where it is equipped, `tooltip`).
            if !occupied(data, i) {
                let label = if *cosmetic { "accessories.cosmetic_slot.tooltip.singular" } else { "accessories.slot.tooltip.singular" };
                lines.push(format!("{}{name}", tr(game, label)));
            }
            return;
        }
    }
}

// ---- the client: the key, the inventory's button, the tick -------------------------------

/// The player's own inventory screen, or creative's inventory tab
/// (`menuButtonInjections`: `player_inventory`, `creative_player_inventory`).
fn inventory_shown(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>) -> bool {
    screen.id == "inventory" && screen.menu.is_none() && !game.creative_tab_open()
}

/// The 8x8 button that opens the screen, at its offset from the panel
/// (`player_inventory` 66, 8; `creative_player_inventory` 96, 6).
fn open_button(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>) -> [f32; 4] {
    let menu = if game.creative_inventory_open() { "minecraft:creative_player_inventory" } else { "minecraft:player_inventory" };
    let (dx, dy) = config().menu_button_injections.iter().find(|(m, _, _)| m == menu).map_or((-1000.0, -1000.0), |(_, x, y)| (*x, *y));
    [screen.rect[0] + dx, screen.rect[1] + dy, 8.0, 8.0]
}

/// Drawn over any screen (`ScreenEvent.Render.Post`): the inventory's button.
pub(super) fn screen_render_post(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
    let ours = our_screen(screen);
    TOOLTIP_SCREEN.with(|t| t.set(ours));
    if !ours {
        TOGGLE_HOVERED.with(|t| t.set(false));
        HOVERED_SLOT.with(|h| *h.borrow_mut() = None);
    }
    if !inventory_shown(game, screen) {
        return;
    }
    let r = open_button(game, screen);
    let over = inside(r, screen.mouse);
    let name = if over { "accessories:gui/accessories_open_icon_hovered" } else { "accessories:gui/accessories_open_icon" };
    game.native_sprite(batch, name, [0.0, 0.0, 8.0, 8.0], r);
    if over {
        let line = Text::literal(tr(game, "accessories.open.screen"));
        game.draw_tooltip_lines(batch, &[(line, u32::MAX)], screen.mouse[0], screen.mouse[1]);
    }
}

/// A press on any screen: the inventory's button, or the accessories screen's own.
pub(super) fn screen_pressed(game: &Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if *canceled {
        return;
    }
    if our_screen(screen) {
        screen_press(screen, button, canceled);
        return;
    }
    if button == 0 && inventory_shown(game, screen) && inside(open_button(game, screen), screen.mouse) {
        *canceled = true;
        OPEN_ASKED.with(|o| o.set(true));
    }
}

/// The key (H): opens the screen while playing or on the inventory, closes it when it is open.
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || key != game.native_mod_key(MOD_ID, "open_accessories_screen", "key.keyboard.h") {
        return;
    }
    let ours = game.native_screen_shown() && super::native_hooks::accessories_screen::menu(game).is_some_and(|m| m == MENU);
    if ours {
        game.close_container();
        game.screen = super::Screen::Playing;
        game.capture(true);
        return;
    }
    if game.native_playing() || matches!(game.screen, super::Screen::Inventory) {
        let _ = game.native_net_send_to_server(OPEN_CHANNEL, &[]);
    }
}

/// The client's tick: what the screen asked goes to the server, the slots
/// move to the layout, the back button goes back to the inventory.
pub(super) fn client_tick(game: &mut Game<'_>) {
    if OPEN_ASKED.with(|o| o.replace(false)) {
        let _ = game.native_net_send_to_server(OPEN_CHANNEL, &[]);
    }
    let asks = ASKS.with(|q| std::mem::take(&mut *q.borrow_mut()));
    // The player's equip control, once a server told its slots.
    if SEND_EQUIP_CONTROL.with(|s| s.replace(false)) {
        let value = config().equip_control;
        let _ = game.native_net_send_to_server(OPTION_CHANNEL, format!("equip_control={value}").as_bytes());
    }
    let shown = game.native_screen_shown() && super::native_hooks::accessories_screen::menu(game).is_some_and(|m| m == MENU);
    let mut relayout = false;
    for a in asks {
        match a {
            Ask::Toggle(i) => {
                let _ = game.native_net_send_to_server(TOGGLE_CHANNEL, &(i as u16).to_le_bytes());
            }
            Ask::Option(name, value) => {
                let _ = game.native_net_send_to_server(OPTION_CHANNEL, format!("{name}={value}").as_bytes());
            }
            Ask::Relayout => relayout = shown,
            Ask::Back if !shown => {}
            Ask::Back => {
                // `backToPreviousScreen`: the inventory again (`backButtonClosesScreen`: closed).
                game.close_container();
                if config().back_button_closes_screen {
                    game.screen = super::Screen::Playing;
                    game.capture(true);
                    return;
                }
                if game.creative {
                    game.screen = super::Screen::Creative;
                } else {
                    game.screen = super::Screen::Inventory;
                }
                game.craft_size = 2;
                game.capture(false);
                return;
            }
        }
    }
    if relayout {
        super::native_hooks::accessories_screen::relayout(game);
    }
}

/// The server told what a player wears (`SyncEntireContainer`).
pub(super) fn net_from_server(channel: &str, data: &[u8]) {
    if channel == SLOTS_CHANNEL {
        slots_received(data);
        SEND_EQUIP_CONTROL.with(|s| s.set(true));
        return;
    }
    if channel != WORN_CHANNEL {
        return;
    }
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let mut lines = text.lines();
    let Some(name) = lines.next() else {
        return;
    };
    let mut worn = Vec::new();
    for line in lines {
        let mut parts = line.splitn(4, '\t');
        let (Some(slot), Some(index), Some(cosmetic), Some(item)) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let read = |snbt: &str| horizon_world::nbt::parse_snbt(snbt).ok().and_then(|(t, _)| horizon_items::item_nbt::item_from_nbt(&t));
        // `showCosmeticAccessories`: the cosmetic over the accessory.
        let stack = if config().show_cosmetic_accessories { read(cosmetic).or_else(|| read(item)) } else { read(item) };
        let Some(stack) = stack else {
            continue;
        };
        worn.push((slot.to_owned(), index.parse().unwrap_or(0), stack));
    }
    WEARING.with(|w| {
        w.borrow_mut().insert(name.to_owned(), worn);
    });
}

// ---- the client: accessories on the player (`AccessoriesRenderLayer`) --------------------

pub(super) fn render_listened(listened: &mut bool) {
    *listened = true;
}

/// A part of the player model (`HumanoidModel`): its pivot and its box, in
/// the model's pixels (y down, -z the front).
struct Part {
    pivot: [f32; 3],
    min: [f32; 3],
    max: [f32; 3],
    /// `xRot`, `yRot`, `zRot` in radians.
    rot: [f32; 3],
}

/// The player model's parts as `HumanoidModel.setupAnim` poses them (no crouch or arm pose).
fn part(mob: &super::native_mob_draw::NativeMobRender<'_>, name: &str) -> Part {
    let (pos, speed) = (mob.walk_position, mob.walk_speed.min(1.0));
    let pi = std::f32::consts::PI;
    let swing = |phase: f32, scale: f32| (pos * 0.6662 + phase).cos() * scale * speed;
    let bob_z = (mob.age * 0.09).cos() * 0.05 + 0.05;
    let bob_x = (mob.age * 0.067).sin() * 0.05;
    match name {
        "head" => Part { pivot: [0.0; 3], min: [-4.0, -8.0, -4.0], max: [4.0, 0.0, 4.0], rot: [mob.pitch.to_radians(), mob.head_yaw.to_radians(), 0.0] },
        "right_arm" => Part {
            pivot: [-5.0, 2.0, 0.0],
            min: [-3.0, -2.0, -2.0],
            max: [1.0, 10.0, 2.0],
            rot: [swing(pi, 1.0) + bob_x, 0.0, bob_z],
        },
        "left_arm" => Part {
            pivot: [5.0, 2.0, 0.0],
            min: [-1.0, -2.0, -2.0],
            max: [3.0, 10.0, 2.0],
            rot: [swing(0.0, 1.0) - bob_x, 0.0, -bob_z],
        },
        "right_leg" => Part { pivot: [-1.9, 12.0, 0.0], min: [-2.0, 0.0, -2.0], max: [2.0, 12.0, 2.0], rot: [swing(0.0, 1.4), 0.0, 0.0] },
        "left_leg" => Part { pivot: [1.9, 12.0, 0.0], min: [-2.0, 0.0, -2.0], max: [2.0, 12.0, 2.0], rot: [swing(pi, 1.4), 0.0, 0.0] },
        _ => Part { pivot: [0.0; 3], min: [-4.0, 0.0, -2.0], max: [4.0, 12.0, 2.0], rot: [0.0; 3] },
    }
}

/// `ModelTransformOps.transformToModelPart`: into the part (`translateAndRotate`),
/// then to the point `percent` of its box (1 the min side, -1 the max), half
/// a block a unit, turned over (`XP 180`).
fn to_part(draw: &mut super::native_block_entity_draw::BlockEntityDraw, p: &Part, percent: [f32; 3]) {
    draw.translate([p.pivot[0] / 16.0, p.pivot[1] / 16.0, p.pivot[2] / 16.0]);
    // `rotationZYX(z, y, x)`.
    draw.rotate_about([0.0, 0.0, 1.0], p.rot[2].to_degrees(), [0.0; 3]);
    draw.rotate_about([0.0, 1.0, 0.0], p.rot[1].to_degrees(), [0.0; 3]);
    draw.rotate_about([1.0, 0.0, 0.0], p.rot[0].to_degrees(), [0.0; 3]);
    draw.scale([1.0 / 16.0; 3]);
    let lerp = |pc: f32, a: f32, b: f32| a + (b - a) * ((-pc + 1.0) / 2.0);
    draw.translate([lerp(percent[0], p.min[0], p.max[0]), lerp(percent[1], p.min[1], p.max[1]), lerp(percent[2], p.min[2], p.max[2])]);
    draw.scale([8.0; 3]);
    draw.rotate_about([1.0, 0.0, 0.0], 180.0, [0.0; 3]);
}

/// `DefaultAccessoryRenderer`: each slot's accessory where its helper puts it
/// (none for `charm` or an unknown slot).
pub(super) fn mob_layer(
    game: &Game<'_>,
    mob: &super::native_mob_draw::NativeMobRender<'_>,
    who: Option<&super::native_mob_draw::NativePlayerRender<'_>>,
    draw: &mut super::native_block_entity_draw::BlockEntityDraw,
) {
    use horizon_assets::item_model::DisplayContext;
    let Some(who) = who else {
        return;
    };
    let Some(worn) = WEARING.with(|w| w.borrow().get(who.name).cloned()) else {
        return;
    };
    let disabled = config().disabled_default_renders;
    for (slot, index, stack) in worn {
        let block = game.items().get(&stack.item).is_some_and(|i| i.block.is_some());
        if disabled.iter().any(|(s, t)| *s == slot && (t == "ALL" || (t == "BLOCK") == block)) {
            continue;
        }
        // Banners and gliders have no renderer of their own
        // (`AccessoriesRendererRegistry` binds them to none): a banner in the
        // hat slot is worn on the head as `CustomHeadLayer` draws it.
        let banner = stack.item.starts_with("minecraft:") && stack.item.ends_with("_banner");
        if banner || game.items().get(&stack.item).is_some_and(|i| i.glider) {
            if banner && slot == "hat" {
                draw.push();
                draw.translate([0.0, 1.501, 0.0]);
                draw.rotate_about([1.0, 0.0, 0.0], 180.0, [0.0; 3]);
                let head = part(mob, "head");
                draw.translate([head.pivot[0] / 16.0, head.pivot[1] / 16.0, head.pivot[2] / 16.0]);
                draw.rotate_about([0.0, 1.0, 0.0], head.rot[1].to_degrees(), [0.0; 3]);
                draw.rotate_about([1.0, 0.0, 0.0], head.rot[0].to_degrees(), [0.0; 3]);
                // `translate(0, -0.25, 0)`, `YP 180`, `scale(0.625, -0.625, -0.625)`.
                draw.translate([0.0, -0.25, 0.0]);
                draw.rotate_about([0.0, 1.0, 0.0], 180.0, [0.0; 3]);
                draw.rotate_about([1.0, 0.0, 0.0], 180.0, [0.0; 3]);
                draw.scale([0.625; 3]);
                game.native_draw_item(draw, &stack, DisplayContext::Head, None, 0);
                draw.pop();
            }
            continue;
        }
        let side = |even: &'static str, odd: &'static str| if index % 2 == 0 { even } else { odd };
        let item = |draw: &mut super::native_block_entity_draw::BlockEntityDraw| {
            game.native_draw_item(draw, &stack, DisplayContext::None, None, 0);
        };
        draw.push();
        // The model's frame: 1.501 up from the feet, turned over (`scale(-1, -1, 1)` and the body's half turn).
        draw.translate([0.0, 1.501, 0.0]);
        draw.rotate_about([1.0, 0.0, 0.0], 180.0, [0.0; 3]);
        match slot.as_str() {
            "face" => {
                to_part(draw, &part(mob, "head"), [0.0, 0.0, 1.0]);
                item(draw);
            }
            "hat" => {
                to_part(draw, &part(mob, "head"), [0.0, 1.0, 0.0]);
                draw.translate([0.0, 0.25, 0.0]);
                for _ in 0..stack.count {
                    item(draw);
                    draw.translate([0.0, 0.5, 0.0]);
                }
            }
            "back" => {
                to_part(draw, &part(mob, "body"), [0.0, 0.0, -1.0]);
                draw.scale([1.5; 3]);
                item(draw);
            }
            "necklace" | "cape" => {
                let z = if slot == "necklace" { 1.0 } else { -1.0 };
                to_part(draw, &part(mob, "body"), [0.0, 1.0, z]);
                draw.translate([0.0, -0.25, 0.0]);
                item(draw);
            }
            "ring" => {
                let x = if index % 2 == 0 { 1.0 } else { -1.0 };
                to_part(draw, &part(mob, side("right_arm", "left_arm")), [x, -1.0, 0.0]);
                let offset = (index / 2) as f32;
                draw.translate([-x * offset * -0.0001, 0.25 * (offset + 1.0), 0.0]);
                draw.scale([0.5; 3]);
                draw.rotate_about([0.0, 1.0, 0.0], 90.0, [0.0; 3]);
                for _ in 0..stack.count {
                    item(draw);
                    draw.translate([0.0, 0.0, if index % 2 == 0 { -0.5 } else { 0.5 }]);
                }
            }
            "wrist" => {
                to_part(draw, &part(mob, side("right_arm", "left_arm")), [0.0, -0.5, 0.0]);
                draw.scale([1.01; 3]);
                draw.rotate_about([0.0, 1.0, 0.0], 90.0, [0.0; 3]);
                item(draw);
            }
            "hand" => {
                to_part(draw, &part(mob, side("right_arm", "left_arm")), [0.0, -1.0, 0.0]);
                draw.translate([0.0, 0.25, 0.0]);
                draw.scale([1.02; 3]);
                draw.rotate_about([0.0, 1.0, 0.0], 90.0, [0.0; 3]);
                item(draw);
            }
            "belt" => {
                to_part(draw, &part(mob, "body"), [0.0, -1.0, 0.0]);
                draw.scale([1.01; 3]);
                item(draw);
            }
            "anklet" => {
                to_part(draw, &part(mob, side("right_leg", "left_leg")), [0.0, -0.5, 0.0]);
                draw.scale([1.01; 3]);
                item(draw);
            }
            "shoes" => {
                for leg in ["right_leg", "left_leg"] {
                    draw.push();
                    to_part(draw, &part(mob, leg), [0.0, -1.0, 0.0]);
                    draw.translate([0.0, 0.25, 0.0]);
                    draw.scale([1.02; 3]);
                    item(draw);
                    draw.pop();
                }
            }
            _ => {}
        }
        draw.pop();
    }
}

// ---- Options > Mods and Mod Menu ---------------------------------------------------------

/// The mod's row: its key listed on its page, where the player sets it.
pub(super) fn mods(mods: &mut Vec<super::native_hooks::NativeMod>) {
    let label = "accessories.key.open_accessories_screen";
    mods.push(super::native_hooks::NativeMod {
        id: MOD_ID.into(),
        title: format!("Accessories {VERSION}"),
        description: if horizon_client::lang::code().starts_with("ru") {
            "Слоты аксессуаров, настраиваемые данными. Порт Accessories.".into()
        } else {
            "An extendable and data-driven Accessory Mod. A port of Accessories.".into()
        },
        keys: vec![["open_accessories_screen".into(), horizon_client::lang::get_or(label, "Open Accessories Screen"), "key.keyboard.h".into()]],
    });
}

thread_local! {
    static MOD_MENU_WRITTEN: Cell<bool> = const { Cell::new(false) };
}

/// Mod Menu's icon and metadata (`fabric.mod.json`) in the mod's folder, each
/// written when it differs, once a run.
fn write_mod_menu_files(game: &Game<'_>) {
    if MOD_MENU_WRITTEN.with(|w| w.replace(true)) {
        return;
    }
    const ICON: &[u8] = horizon_client::native_data!("accessories_native", "icon.png");
    let dir = game.native_data_dir(MOD_ID);
    let _ = std::fs::create_dir_all(&dir);
    let json = format!(
        r#"{{
  "schema": 1,
  "name": "Accessories",
  "version": "{VERSION}",
  "description": "An extendable and data-driven Accessory Mod for Minecraft",
  "description_ru": "Расширяемый мод слотов аксессуаров, настраиваемых данными. Порт Accessories.",
  "authors": ["blodhgarm", "chyzman", "Dr.Zeal"],
  "contributors": ["glisco", "BasiqueEvangelist", "mpustovoi", "EvanHsieh0415", "notlin4", "RuyaSavascisi", "TheLegendofSaram", "DispatchedToHeaven (Horizon port)", "DeportedToHell"],
  "license": "MIT",
  "links": {{
    "homepage": "https://github.com/wisp-forest/accessories",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/accessories",
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

fn tr(game: &Game<'_>, key: &str) -> String {
    game.translate(key, key)
}

/// `%s` and `%1$s` filled from `args`.
fn format_args(template: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut next = 0;
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut spec = String::new();
        while let Some(&d) = chars.peek() {
            if d.is_ascii_digit() || d == '$' {
                spec.push(d);
                chars.next();
            } else {
                break;
            }
        }
        match chars.next() {
            Some('s' | 'd') => {
                let k = spec.strip_suffix('$').and_then(|n| n.parse::<usize>().ok()).map_or_else(
                    || {
                        next += 1;
                        next - 1
                    },
                    |n| n - 1,
                );
                out.push_str(args.get(k).map_or("", String::as_str));
            }
            Some('%') => out.push('%'),
            Some(other) => {
                out.push('%');
                out.push_str(&spec);
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    out
}
