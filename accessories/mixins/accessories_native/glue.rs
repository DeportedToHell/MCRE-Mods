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
const VERSION: &str = "0.1.0";
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
        let amount = (b.base.unwrap_or(1).max(0) + b.offset).max(0) as usize;
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
/// as many as the slot type's amount.
#[derive(Clone, Debug, Default, PartialEq)]
struct Container {
    render: Vec<bool>,
    items: Vec<Option<ItemStack>>,
    cosmetics: Vec<Option<ItemStack>>,
}

impl Container {
    fn sized(size: usize) -> Self {
        Self { render: vec![true; size], items: vec![None; size], cosmetics: vec![None; size] }
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

/// Player `uuid`'s accessories, sized by the data: what no longer fits (a
/// smaller slot, a slot they lost) is in the second list, to give back.
/// None when the player isn't reached.
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
    let mut invalid = Vec::new();
    let mut holder = Holder::new();
    with_data(|d| {
        for name in &d.player {
            let size = d.slot(name).map_or(0, |t| t.amount);
            let mut c = Container::sized(size);
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
        sc.insert(jkey("base_size"), Tag::Int(c.items.len() as i32));
        sc.insert(jkey("current_size"), Tag::Int(c.items.len() as i32));
        sc.insert(jkey("render_options"), Tag::List(c.render.iter().map(|&b| Tag::Byte(i8::from(b))).collect()));
        sc.insert(jkey("items"), stacks_to(&c.items));
        sc.insert(jkey("cosmetics"), stacks_to(&c.cosmetics));
        containers.insert(jkey(name), Tag::Compound(sc));
    }
    let mut h = Compound::new();
    h.insert(jkey("accessories_containers"), Tag::Compound(containers));
    data.insert(jkey(HOLDER), Tag::Compound(h));
    game.native_set_player_persistent_data(uuid, Tag::Compound(data))
}

// ---- screen options (`PlayerOptions`) ----------------------------------------------------

/// The accessories screen's options for a player, kept on the server.
#[derive(Clone, Debug, PartialEq)]
struct Options {
    /// Cosmetic slots shown instead of the accessories (`COSMETIC`).
    cosmetic: bool,
    /// Slots no item fits shown too (`UNUSED_SLOTS`).
    unused_slots: bool,
    /// Columns of the accessories grid (`COLUMN_AMOUNT`).
    columns: i32,
    /// The group filter beside the entity (`GROUP_FILTER`).
    group_filter: bool,
    /// Groups the grid is narrowed to (`FILTERED_GROUPS`); none: all.
    filtered_groups: Vec<String>,
    /// Dark theme (`isDarkMode`).
    dark: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { cosmetic: false, unused_slots: false, columns: 2, group_filter: true, filtered_groups: Vec::new(), dark: false }
    }
}

impl Options {
    fn encode(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            u8::from(self.cosmetic),
            u8::from(self.unused_slots),
            self.columns,
            u8::from(self.group_filter),
            u8::from(self.dark),
            self.filtered_groups.join(",")
        )
    }
    fn decode(text: &str) -> Self {
        let mut o = Self::default();
        let parts: Vec<&str> = text.split('\t').collect();
        let flag = |i: usize, d: bool| parts.get(i).map_or(d, |v| *v == "1");
        o.cosmetic = flag(0, false);
        o.unused_slots = flag(1, false);
        o.columns = parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(2).clamp(1, 9);
        o.group_filter = flag(3, true);
        o.dark = flag(4, false);
        o.filtered_groups = parts.get(5).map(|v| v.split(',').filter(|g| !g.is_empty()).map(str::to_owned).collect()).unwrap_or_default();
        o
    }
    /// `name=value` changes it (a client's `SyncOptionChange`).
    fn set(&mut self, name: &str, value: &str) {
        let on = value == "1" || value == "true";
        match name {
            "cosmetic" => self.cosmetic = on,
            "unused_slots" => self.unused_slots = on,
            "column_amount" => self.columns = value.parse().unwrap_or(2).clamp(1, 9),
            "group_filter" => self.group_filter = on,
            "dark" => self.dark = on,
            "filtered_groups" => self.filtered_groups = value.split(',').filter(|g| !g.is_empty()).map(str::to_owned).collect(),
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

/// Where the config allows banners, gliders and totems (the shipped defaults).
struct ContentOptions {
    allow_banner_equip: bool,
    valid_banner_slots: Vec<String>,
    allow_glider_equip: bool,
    valid_glider_slots: Vec<String>,
    allow_totem_equip: bool,
    valid_totem_slots: Vec<String>,
}

fn content_options() -> ContentOptions {
    let list = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect();
    ContentOptions {
        allow_banner_equip: true,
        valid_banner_slots: list(&["cape", "hat"]),
        allow_glider_equip: false,
        valid_glider_slots: list(&["cape", "back"]),
        allow_totem_equip: false,
        valid_totem_slots: list(&["charm"]),
    }
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
    opening: Vec<u128>,
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

/// Opens player `uuid`'s accessories menu (`AccessoriesMenu`).
fn open_menu(game: &mut Game<'_>, uuid: u128) {
    if with(|s| s.open.contains_key(&uuid)) {
        return;
    }
    let Some((holder, invalid)) = read_holder(game, uuid) else {
        return;
    };
    for stack in invalid {
        give_back(game, uuid, stack);
    }
    write_holder(game, uuid, &holder);
    let options = read_options(game, uuid);
    let (entries, rules) = menu_layout(&holder);
    let Some((conn, armor, offhand, carried)) = game
        .lan
        .as_ref()
        .and_then(|h| h.hub.guests.iter().find(|g| g.placed && g.uuid == uuid))
        .map(|g| (g.conn, g.armor.clone(), g.offhand.clone(), g.inventory.iter().take(36).flatten().cloned().collect::<Vec<_>>()))
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
            s.open.insert(uuid, Open { window, entries, last: items, mask: Vec::new() });
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
    if let Some(g) = host.hub.guests.iter_mut().find(|g| g.uuid == uuid) {
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
        write_back(game, uuid, &open, &items);
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
fn closed_menu(game: &mut Game<'_>, uuid: u128, open: &Open, items: &[Option<ItemStack>]) {
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
    let mut one = held.clone();
    one.count = 1;
    let old = holder.get_mut(&slot).and_then(|c| c.items.get_mut(i)).and_then(|place| place.replace(one.clone()));
    if !write_holder(game, uuid, &holder) {
        return;
    }
    // The hand gives one; what came off goes into it, else into the inventory.
    let mut rest = held;
    rest.count -= 1;
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
            let Some(stack) = c.cosmetics.get(i).cloned().flatten().or_else(|| c.items.get(i).cloned().flatten()) else {
                continue;
            };
            let snbt = horizon_world::snbt_text::to_snbt(&horizon_items::item_nbt::item_to_nbt(&stack));
            out.push_str(&format!("{name}\t{i}\t{snbt}\n"));
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
        closed_menu(game, uuid, &open, &items);
        // The armour and off hand may have changed under the menu: the
        // inventory menu (window 0) is told again, as it is on closing.
        if let Some(host) = game.lan.as_mut()
            && let Some(i) = host.hub.guests.iter().position(|g| g.uuid == uuid)
        {
            host.hub.send_inventory(i);
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
        with(|s| s.told.clear());
    }
    if !with(|s| std::mem::take(&mut s.joined)).is_empty() {
        with(|s| s.told.clear());
    }
    for (uuid, name, value) in with(|s| std::mem::take(&mut s.options)) {
        let mut options = read_options(game, uuid);
        options.set(&name, &value);
        write_options(game, uuid, &options);
    }
    for (uuid, slot) in with(|s| std::mem::take(&mut s.toggles)) {
        let Some(Entry::Slot(name, i, _)) = with(|s| s.open.get(&uuid).and_then(|o| o.entries.get(slot).cloned())) else {
            continue;
        };
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
    for uuid in with(|s| std::mem::take(&mut s.opening)) {
        open_menu(game, uuid);
    }
    tell_worn(game);
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
            if !s.opening.contains(&player) {
                s.opening.push(player);
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
    if *claimed || game.native_sneaking() {
        return;
    }
    let uuid = game.native_acting_id();
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
    /// The first row of the accessories grid shown.
    scroll: usize,
}

/// Where things are on the screen, from its top left.
#[derive(Clone, Debug, Default)]
struct Geo {
    width: f32,
    height: f32,
    /// The accessories grid's panel, its columns and rows shown, and all its rows.
    grid: [f32; 4],
    rows: usize,
    total_rows: usize,
    /// The group filter's panel, if shown.
    side: Option<[f32; 4]>,
    entity: [f32; 2],
    /// The inventory's background.
    inventory: [f32; 2],
    /// Each menu slot's place, if shown.
    places: Vec<Option<[f32; 2]>>,
    /// The grid's slots shown, by menu slot, with their cell.
    cells: Vec<(usize, [f32; 2])>,
}

const ROWS_SHOWN: usize = 7;
const ENTITY_W: f32 = 122.0;
const TOP_H: f32 = 140.0;
const INVENTORY_W: f32 = 198.0;
const INVENTORY_H: f32 = 90.0;

impl Screen {
    /// The grid's slots: the accessories or the cosmetics (`Switch`), used
    /// ones unless unused are shown, in the groups filtered to.
    fn grid_slots(&self) -> Vec<usize> {
        let o = &self.options;
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| match s {
                Shown::Slot { cosmetic, group, used, .. } => {
                    *cosmetic == o.cosmetic && (*used || o.unused_slots) && (o.filtered_groups.is_empty() || o.filtered_groups.contains(group))
                }
                _ => false,
            })
            .map(|(i, _)| i)
            .collect()
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
    fn geo(&self) -> Geo {
        let grid = self.grid_slots();
        let columns = (self.options.columns.max(1)) as usize;
        let total_rows = grid.len().div_ceil(columns).max(1);
        let rows = total_rows.min(ROWS_SHOWN);
        let scrolls = total_rows > ROWS_SHOWN;
        let grid_w = columns as f32 * 18.0 + 14.0 + if scrolls { 10.0 } else { 0.0 };
        let grid_h = rows as f32 * 18.0 + 14.0;
        let groups = self.filter_groups();
        let side = (self.options.group_filter && !groups.is_empty()).then(|| {
            let h = 12.0 + 14.0 * (groups.len() + 1) as f32 + 2.0 * groups.len() as f32;
            [26.0, h.min(TOP_H)]
        });
        // `ENTITY_CENTERED`: the entity panel in the middle of the row.
        let left = grid_w + 2.0;
        let right = side.map_or(0.0, |[w, _]| w + 2.0);
        let pad = left.max(right);
        let width = (pad * 2.0 + ENTITY_W).max(INVENTORY_W + 22.0);
        let height = TOP_H + 2.0 + INVENTORY_H;
        let ex = (width - ENTITY_W) / 2.0;
        let gx = ex - 2.0 - grid_w;
        let gy = (TOP_H - grid_h) / 2.0;
        let side = side.map(|[w, h]| [ex + ENTITY_W + 2.0, (TOP_H - h) / 2.0, w, h]);
        let comp_x = (width - 176.0) / 2.0;
        let inventory = [comp_x - 22.0, TOP_H + 2.0];
        let scroll = self.scroll.min(total_rows.saturating_sub(rows));
        let mut places = vec![None; self.slots.len()];
        let mut cells = Vec::new();
        for (v, &i) in grid.iter().enumerate() {
            let (row, col) = (v / columns, v % columns);
            if row < scroll || row >= scroll + rows {
                continue;
            }
            let cell = [gx + 7.0 + col as f32 * 18.0, gy + 7.0 + (row - scroll) as f32 * 18.0];
            places[i] = Some([cell[0] + 1.0, cell[1] + 1.0]);
            cells.push((i, cell));
        }
        for (i, s) in self.slots.iter().enumerate() {
            match s {
                Shown::Armor(k) => places[i] = Some([ex + 8.0, 31.0 + 18.0 * *k as f32]),
                Shown::CosArmor { k, .. } => places[i] = Some([ex + 98.0, 31.0 + 18.0 * *k as f32]),
                Shown::Offhand => places[i] = Some([inventory[0] + 8.0, inventory[1] + 66.0]),
                _ => {}
            }
        }
        Geo {
            width,
            height,
            grid: [gx, gy, grid_w, grid_h],
            rows,
            total_rows,
            side,
            entity: [ex, 0.0],
            inventory,
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
    /// What each player wears, by profile name, as the server told.
    static WEARING: RefCell<HashMap<String, Vec<(String, usize, ItemStack)>>> = RefCell::new(HashMap::new());
}

fn ask(a: Ask) {
    ASKS.with(|q| q.borrow_mut().push(a));
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

/// The screen's size and where each slot is (`native_screen`).
pub(super) fn screen(menu: &str, out: &mut Option<NativeScreen>) {
    if menu != MENU {
        return;
    }
    let Some(screen) = SCREEN.with(|s| s.borrow().clone()) else {
        return;
    };
    let geo = screen.geo();
    let mut slots: Vec<Option<[f32; 2]>> = geo.places.clone();
    slots.resize(256, None);
    *out = Some(NativeScreen {
        width: geo.width,
        height: geo.height,
        slots,
        inventory_y: geo.inventory[1] + 8.0,
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


/// A 14x14 (or 10x10) icon button, its hovered look under the pointer.
fn icon_button(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, icon: &str, size: f32, [x, y]: [f32; 2], mouse: [f32; 2]) -> bool {
    let over = inside([x, y, size, size], mouse);
    let name = if over { format!("accessories:gui/{icon}_hovered") } else { format!("accessories:gui/{icon}") };
    game.native_sprite(batch, &name, [0.0, 0.0, size, size], [x, y, size, size]);
    over
}

/// The corner buttons of the entity panel (`cosmetic`, `back`), from the panel's top left.
fn corner_buttons(geo: &Geo) -> [[f32; 4]; 2] {
    let [ex, ey] = geo.entity;
    [[ex + 9.0, ey + 9.0, 14.0, 14.0], [ex + ENTITY_W - 19.0, ey + 9.0, 10.0, 10.0]]
}

/// A slot's 5x5 render toggle, at its cell's top right (`ToggleButton`).
fn toggle_rect(cell: [f32; 2]) -> [f32; 4] {
    [cell[0] + 14.0, cell[1] - 1.0, 5.0, 5.0]
}

/// The occupied menu slots, from the data slots (16 a word).
fn occupied(data: &[i32], i: usize) -> bool {
    data.get(i / 16).is_some_and(|w| (*w >> (i % 16)) & 1 == 1)
}

/// Draws the accessories screen under its slots' items (`native_screen_draw`).
pub(super) fn screen_draw(game: &Game<'_>, menu: &str, data: &[i32], batch: &mut horizon_client::hud::HudBatch, origin: [f32; 2]) {
    if menu != MENU {
        return;
    }
    let Some(screen) = SCREEN.with(|s| s.borrow().clone()) else {
        return;
    };
    let geo = screen.geo();
    let dark = screen.options.dark;
    let t = theme(dark);
    let [ox, oy] = origin;
    let cursor = game.native_cursor();
    let mouse = [cursor[0] - ox, cursor[1] - oy];
    let at = |[x, y]: [f32; 2]| [ox + x, oy + y];
    // The inventory's background, with the off hand's slot at its left.
    let [ix, iy] = geo.inventory;
    game.native_sprite(
        batch,
        &format!("accessories:gui/theme/{t}/player_inv/without_crafting"),
        [0.0, 0.0, INVENTORY_W, INVENTORY_H],
        [ox + ix, oy + iy, INVENTORY_W, INVENTORY_H],
    );
    // The entity panel: background, the player, the frame with its armour columns.
    let [ex, ey] = geo.entity;
    game.native_sprite(
        batch,
        &format!("accessories:gui/theme/{t}/entity_view/single/entity_background"),
        [0.0, 0.0, 108.0, 126.0],
        [ox + ex + 7.0, oy + ey + 7.0, 108.0, 126.0],
    );
    game.draw_player_preview(batch, ox + ex + 7.0, oy + ey + 7.0, ox + ex + 115.0, oy + ey + 133.0, 50.0, 0.0625);
    game.native_sprite(
        batch,
        &format!("accessories:gui/theme/{t}/entity_view/single/4_slots"),
        [0.0, 0.0, ENTITY_W, TOP_H],
        [ox + ex, oy + ey, ENTITY_W, TOP_H],
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
    // The accessories grid (`AccessoriesContainingComponent`).
    panel(game, batch, dark, [ox + geo.grid[0], oy + geo.grid[1], geo.grid[2], geo.grid[3]]);
    for &(i, cell) in &geo.cells {
        let [cx, cy] = at(cell);
        if dark {
            game.native_sprite(batch, "accessories:gui/theme/dark/slot", [0.0, 0.0, 18.0, 18.0], [cx, cy, 18.0, 18.0]);
        } else {
            game.native_sprite(batch, "gui/sprites/container/slot", [0.0, 0.0, 18.0, 18.0], [cx, cy, 18.0, 18.0]);
        }
        if let Some(Shown::Slot { icon, render, .. }) = screen.slots.get(i) {
            if !occupied(data, i) {
                game.native_sprite(batch, icon, [0.0, 0.0, 16.0, 16.0], [cx + 1.0, cy + 1.0, 16.0, 16.0]);
            }
            if !screen.options.cosmetic {
                draw_toggle(game, batch, dark, *render, [cx, cy], mouse, cell);
            }
        }
    }
    // The grid's scroll bar, when it has more rows than it shows.
    if geo.total_rows > geo.rows {
        let [gx, gy, gw, gh] = geo.grid;
        let track = [ox + gx + gw - 7.0 - 8.0, oy + gy + 7.0, 8.0, gh - 14.0];
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
    let [cosmetic, back] = corner_buttons(&geo);
    let icon = if screen.options.cosmetic { "charm_toggle_icon" } else { "cosmetic_toggle_icon" };
    icon_button(game, batch, icon, 14.0, at([cosmetic[0], cosmetic[1]]), cursor);
    icon_button(game, batch, "accessories_back_icon", 10.0, at([back[0], back[1]]), cursor);
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

/// A press on the accessories screen: its buttons and toggles take it before the slots.
fn screen_press(screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if button != 0 {
        return;
    }
    let Some(s) = SCREEN.with(|s| s.borrow().clone()) else {
        return;
    };
    let geo = s.geo();
    let at = [screen.mouse[0] - screen.rect[0], screen.mouse[1] - screen.rect[1]];
    let [cosmetic, back] = corner_buttons(&geo);
    if inside(cosmetic, at) {
        *canceled = true;
        let on = !s.options.cosmetic;
        SCREEN.with(|c| {
            if let Some(c) = c.borrow_mut().as_mut() {
                c.options.cosmetic = on;
                c.scroll = 0;
            }
        });
        ask(Ask::Option("cosmetic".into(), u8::from(on).to_string()));
        ask(Ask::Relayout);
        return;
    }
    if inside(back, at) {
        *canceled = true;
        ask(Ask::Back);
        return;
    }
    // Render toggles: the grid's (not on cosmetics) and the cosmetic armour's.
    let mut toggled = None;
    if !s.options.cosmetic {
        for &(i, cell) in &geo.cells {
            if inside(toggle_rect(cell), at) {
                toggled = Some(i);
            }
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
        SCREEN.with(|c| {
            if let Some(c) = c.borrow_mut().as_mut() {
                match c.slots.get_mut(i) {
                    Some(Shown::Slot { render, .. } | Shown::CosArmor { render, .. }) => *render = !*render,
                    _ => {}
                }
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
            set_filter(Vec::new());
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
                set_filter(chosen);
                return;
            }
            by += 16.0;
        }
    }
}

fn set_filter(groups: Vec<String>) {
    SCREEN.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            c.options.filtered_groups = groups.clone();
            c.scroll = 0;
        }
    });
    ask(Ask::Option("filtered_groups".into(), groups.join(",")));
    ask(Ask::Relayout);
}

/// The wheel over the grid scrolls it a row.
pub(super) fn screen_scrolled(screen: &horizon_client::mod_api::NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    if !our_screen(screen) || scroll[1] == 0.0 {
        return;
    }
    let Some(s) = SCREEN.with(|s| s.borrow().clone()) else {
        return;
    };
    let geo = s.geo();
    let at = [screen.mouse[0] - screen.rect[0], screen.mouse[1] - screen.rect[1]];
    if geo.total_rows <= geo.rows || !inside(geo.grid, at) {
        return;
    }
    *canceled = true;
    let max = geo.total_rows - geo.rows;
    let next = if scroll[1] > 0.0 { s.scroll.min(max).saturating_sub(1) } else { (s.scroll + 1).min(max) };
    SCREEN.with(|c| {
        if let Some(c) = c.borrow_mut().as_mut() {
            c.scroll = next;
        }
    });
    ask(Ask::Relayout);
}

/// The tooltip over the screen's own parts: a slot's name, the buttons'.
pub(super) fn screen_tooltip(game: &Game<'_>, menu: &str, data: &[i32], at: [f32; 2], lines: &mut Vec<String>) {
    if menu != MENU {
        return;
    }
    let Some(s) = SCREEN.with(|s| s.borrow().clone()) else {
        return;
    };
    let geo = s.geo();
    let [cosmetic, back] = corner_buttons(&geo);
    if inside(cosmetic, at) {
        let k = if s.options.cosmetic { "accessories.slot_cosmetics.toggle.enabled" } else { "accessories.slot_cosmetics.toggle.disabled" };
        lines.push(tr(game, k));
        return;
    }
    if inside(back, at) {
        lines.push(tr(game, "accessories.back.screen"));
        return;
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
        if !inside([x - 1.0, y - 1.0, 18.0, 18.0], at) || occupied(data, i) {
            continue;
        }
        if let Some(Shown::Slot { slot, cosmetic, render, .. }) = s.slots.get(i) {
            let _ = render;
            // `AccessoriesBasedSlot.getTooltip`: `Slot: <name>`.
            let name = tr(game, &format!("accessories.slot.{}", slot.replace(':', ".")));
            let label = if *cosmetic { "accessories.cosmetic_slot.tooltip.singular" } else { "accessories.slot.tooltip.singular" };
            lines.push(format!("{}{name}", tr(game, label)));
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
    let (dx, dy) = if game.creative_inventory_open() { (96.0, 6.0) } else { (66.0, 8.0) };
    [screen.rect[0] + dx, screen.rect[1] + dy, 8.0, 8.0]
}

/// Drawn over any screen (`ScreenEvent.Render.Post`): the inventory's button.
pub(super) fn screen_render_post(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
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
    if !game.native_screen_shown() || super::native_hooks::accessories_screen::menu(game).is_none_or(|m| m != MENU) {
        return;
    }
    let mut relayout = false;
    for a in asks {
        match a {
            Ask::Toggle(i) => {
                let _ = game.native_net_send_to_server(TOGGLE_CHANNEL, &(i as u16).to_le_bytes());
            }
            Ask::Option(name, value) => {
                let _ = game.native_net_send_to_server(OPTION_CHANNEL, format!("{name}={value}").as_bytes());
            }
            Ask::Relayout => relayout = true,
            Ask::Back => {
                // `backToPreviousScreen`: the inventory again.
                game.close_container();
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
        let mut parts = line.splitn(3, '\t');
        let (Some(slot), Some(index), Some(snbt)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let Some(stack) = horizon_world::nbt::parse_snbt(snbt).ok().and_then(|(t, _)| horizon_items::item_nbt::item_from_nbt(&t)) else {
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
    for (slot, index, stack) in worn {
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
#[allow(dead_code)]
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
