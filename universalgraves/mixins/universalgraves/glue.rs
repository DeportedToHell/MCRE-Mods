// Universal Graves on Horizon, a server mod as the original is: a player's
// death leaves a grave holding their items and experience where they fell
// (`GraveUtils.createGrave`), protected from other players for a while, that
// breaks by itself after a time; the grave is a barrier, as the original shows
// it to every client, with display entities for its model and its text
// (`GraveModelHandler`, the `default` model); it opens a chest menu of its
// items (`GraveGui`), `/graves` lists them (`GraveListGui`), and the owner
// gets a compass to it when they respawn (`GraveCompassItem`). The config is
// the original's file, `config/universal-graves/config.json`, and the graves
// are kept in the world, `data/universal_graves/graves.dat`.
//
// Deaths are heard in the middle of the game's own `guest_die`, with the
// dying player's state swapped in and the hub out of the game's hands: the
// grave is filled there, so nothing drops, and placed at the next tick of the
// level, where every player action heard by a hook is done as well.
// Ported from Universal Graves by Patbox (LGPL-3.0).
use super::Game;
use super::universalgraves_defaults::{DEFAULT_CONFIG, TEXTURES};
use super::universalgraves_text::{self as text, Wrapped};
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_world::nbt::{Compound, JavaString, Tag};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub(super) const ID: &str = "universalgraves";
const NAME: &str = "Universal Graves";
const VERSION: &str = "1.0.0";
/// The original's version this port follows.
const ORIGINAL: &str = "3.13.0+26.3";
/// The grave compass's mark in `minecraft:custom_data` (`universal_graves:compass`).
const COMPASS_KEY: &str = "universal_graves:compass";
/// The player's own data (`LastGraveId`, `HasGraveCompass`) in their persistent data.
const PLAYER_KEY: &str = "universal_graves";
/// The tag the model's display entities carry.
const MODEL_TAG: &str = "universal_graves.model";
/// What a grave shows players: the block the original sends them.
const GRAVE_BLOCK: &str = "minecraft:barrier";
/// The menus' rows (`MenuType.GENERIC_9x5`) and items a page (`PagedGui.PAGE_SIZE`).
const ROWS: u8 = 5;
const PAGE: usize = 36;

fn jkey(s: &str) -> JavaString {
    JavaString::from(s)
}

fn jstr(js: &JavaString) -> String {
    String::from_utf16_lossy(&js.0)
}

fn get_str(c: &Compound, k: &str) -> Option<String> {
    match c.get(&jkey(k)) {
        Some(Tag::String(s)) => Some(jstr(s)),
        _ => None,
    }
}

fn get_i64(c: &Compound, k: &str) -> Option<i64> {
    match c.get(&jkey(k))? {
        Tag::Byte(v) => Some(i64::from(*v)),
        Tag::Short(v) => Some(i64::from(*v)),
        Tag::Int(v) => Some(i64::from(*v)),
        Tag::Long(v) => Some(*v),
        _ => None,
    }
}

fn get_bool(c: &Compound, k: &str) -> Option<bool> {
    get_i64(c, k).map(|v| v != 0)
}

fn sstr(s: &str) -> Tag {
    Tag::String(jkey(s))
}

fn uuid_ints(uuid: u128) -> Vec<i32> {
    horizon_client::lan::playerdata::uuid_ints(uuid).to_vec()
}

fn ints_uuid(v: &[i32]) -> Option<u128> {
    (v.len() == 4).then(|| v.iter().fold(0u128, |acc, &x| (acc << 32) | u128::from(x as u32)))
}

fn nbt_of(v: &Value) -> Tag {
    horizon_client::lan::data::component_nbt(v)
}

fn to_text(v: &Value) -> Text {
    horizon_client::component::to_text(v, &mut |s| Text::literal(s)).with_component(v.clone())
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

// ---- the config (`Config`) ----------------------------------------------------------------

/// `GenericCost`: what unlocking a grave or teleporting to it takes.
#[derive(Clone, Debug)]
struct Cost {
    kind: String,
    input: Option<Value>,
    count: i32,
}

impl Cost {
    fn of(v: &Value) -> Self {
        Self {
            kind: v.get("type").and_then(Value::as_str).unwrap_or("creative").to_owned(),
            input: v.get("input").cloned(),
            count: v.get("count").and_then(Value::as_i64).unwrap_or(1) as i32,
        }
    }
    fn is_free(&self) -> bool {
        self.kind == "free"
    }
    /// `toText`: the type's name, with `× count` unless it is a single one.
    fn text(&self, lang: &str) -> Value {
        let name = match self.kind.as_str() {
            "item" => item_name(self.input.as_ref()),
            k => text::translatable(lang, &format!("text.graves.cost.{k}"), Vec::new()),
        };
        if matches!(self.kind.as_str(), "creative" | "free") {
            name
        } else {
            json!({ "text": "", "extra": [name, { "text": format!(" × {}", self.count) }] })
        }
    }
}

/// An item's name for a cost: its id's own translation key.
fn item_name(input: Option<&Value>) -> Value {
    let id = match input {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(o)) => o.get("id").and_then(Value::as_str).unwrap_or("minecraft:air").to_owned(),
        _ => "minecraft:air".to_owned(),
    };
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", &id));
    json!({ "translate": format!("item.{ns}.{path}") })
}

/// `IconData`: a menu icon's item and its name and lore lines.
#[derive(Clone, Debug)]
struct Icon {
    stack: Value,
    text: Vec<Wrapped>,
}

impl Icon {
    fn of(v: &Value) -> Self {
        let text = match v.get("text") {
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(Wrapped::of).collect(),
            Some(Value::String(s)) => vec![Wrapped::of(s)],
            _ => Vec::new(),
        };
        Self { stack: v.get("icon").cloned().unwrap_or(Value::Null), text }
    }
}

/// `Variant<IconData>`: `base` while active, `alt` otherwise.
#[derive(Clone, Debug)]
struct Variant {
    base: Icon,
    alt: Icon,
}

impl Variant {
    fn of(v: &Value) -> Self {
        Self { base: Icon::of(&v["base"]), alt: Icon::of(&v["alt"]) }
    }
    fn get(&self, active: bool) -> &Icon {
        if active { &self.base } else { &self.alt }
    }
}

#[derive(Clone, Debug)]
struct Config {
    protection_time: i32,
    breaking_time: i32,
    drop_items_on_expiration: bool,
    attackers_bypass: bool,
    use_real_time: bool,
    unlock_cost: Cost,
    give_compass: bool,
    compass_opens_gui: bool,
    click_opens_gui: bool,
    shift_quick_pickup: bool,
    break_quick_pickup: bool,
    remote_protection_removal: bool,
    remote_breaking: bool,
    remote_unlocking: bool,
    xp_type: String,
    xp_percent: f64,
    only_xp: bool,
    blocked_enchantments: Vec<String>,
    grave_limit: i32,
    replace_any: bool,
    max_distance: i32,
    shift_on_failure: bool,
    max_shifts: i32,
    shift_distance: i32,
    on_fluids: bool,
    on_ground: bool,
    visual_after_emptying: bool,
    restore_after_breaking: bool,
    inside_border: bool,
    actively_inside_border: bool,
    ignored_damage: HashMap<String, Wrapped>,
    ignored_attackers: HashMap<String, Wrapped>,
    blacklisted_worlds: HashSet<String>,
    blacklisted_areas: HashMap<String, Vec<[i32; 6]>>,
    failure_text: Wrapped,
    claim_failure_text: Wrapped,
    tp_cost: Cost,
    tp_time: i32,
    tp_height: f64,
    tp_moving: bool,
    tp_timer: Wrapped,
    tp_timer_moving: Wrapped,
    tp_location: Wrapped,
    tp_canceled: Wrapped,
    title: Wrapped,
    admin_title: Wrapped,
    list_icon: Variant,
    admin_list_icon: Variant,
    info_icon: Variant,
    unlock: Variant,
    previous: Variant,
    next: Variant,
    remove_protection: Variant,
    break_grave: Variant,
    quick_pickup: Icon,
    fetch: Variant,
    teleport: Variant,
    back: Icon,
    bar: Icon,
    created: Wrapped,
    protection_ended: Wrapped,
    expired: Wrapped,
    broken: Wrapped,
    no_access: Wrapped,
    accepted: Wrapped,
    payment_failed: Wrapped,
    suffixes: [String; 5],
    infinity: String,
    date_format: String,
    world_names: HashMap<String, Wrapped>,
}

/// Fills `base` with what `over` sets, object by object.
fn merge(base: &mut Value, over: &Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(k) {
                    Some(slot) if slot.is_object() && v.is_object() => merge(slot, v),
                    _ => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, o) => *b = o.clone(),
    }
}

impl Config {
    fn from(v: &Value) -> Self {
        let b = |p: &str, k: &str| v[p][k].as_bool().unwrap_or(false);
        let i = |p: &str, k: &str| v[p][k].as_i64().unwrap_or(0) as i32;
        let w = |p: &str, k: &str| Wrapped::of(v[p][k].as_str().unwrap_or(""));
        let texts = |x: &Value| -> HashMap<String, Wrapped> {
            x.as_object()
                .map(|o| o.iter().map(|(k, t)| (k.clone(), Wrapped::of(t.as_str().unwrap_or("")))).collect())
                .unwrap_or_default()
        };
        let strings = |x: &Value| -> Vec<String> {
            x.as_array().map(|a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default()
        };
        let ui = &v["ui"];
        let t = &v["text"];
        let suffix = |k: &str| t[k].as_str().unwrap_or("").to_owned();
        let mut areas = HashMap::new();
        if let Some(o) = v["placement"]["blacklisted_areas"].as_object() {
            for (world, list) in o {
                let boxes = list
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|x| {
                                let n = |k: &str| x[k].as_i64().unwrap_or(0) as i32;
                                [n("x1"), n("y1"), n("z1"), n("x2"), n("y2"), n("z2")]
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                areas.insert(world.clone(), boxes);
            }
        }
        Self {
            protection_time: i("protection", "non_owner_protection_time"),
            breaking_time: i("protection", "self_destruction_time"),
            drop_items_on_expiration: b("protection", "drop_items_on_expiration"),
            attackers_bypass: b("protection", "attackers_bypass_protection"),
            use_real_time: b("protection", "use_real_time"),
            unlock_cost: Cost::of(&v["interactions"]["unlocking_cost"]),
            give_compass: b("interactions", "give_death_compass"),
            compass_opens_gui: b("interactions", "enable_use_death_compass_to_open_gui"),
            click_opens_gui: b("interactions", "enable_click_to_open_gui"),
            shift_quick_pickup: b("interactions", "shift_and_use_quick_pickup"),
            break_quick_pickup: b("interactions", "break_quick_pickup"),
            remote_protection_removal: b("interactions", "allow_remote_protection_removal"),
            remote_breaking: b("interactions", "allow_remote_breaking"),
            remote_unlocking: b("interactions", "allow_remote_unlocking"),
            xp_type: v["storage"]["experience_type"].as_str().unwrap_or("none").to_owned(),
            xp_percent: v["storage"]["experience_percent:setting_value"].as_f64().unwrap_or(100.0),
            only_xp: b("storage", "can_store_only_xp"),
            blocked_enchantments: strings(&v["storage"]["blocked_enchantments"]),
            grave_limit: i("placement", "player_grave_limit"),
            replace_any: b("placement", "replace_any_block"),
            max_distance: i("placement", "max_distance_from_source_location"),
            shift_on_failure: b("placement", "shift_location_on_failure"),
            max_shifts: i("placement", "max_shift_tries"),
            shift_distance: i("placement", "max_shift_distance"),
            on_fluids: b("placement", "generate_on_top_of_fluids"),
            on_ground: b("placement", "generate_on_ground"),
            visual_after_emptying: b("placement", "create_gravestone_after_emptying"),
            restore_after_breaking: b("placement", "restore_replaced_block_after_player_breaking"),
            inside_border: b("placement", "move_inside_world_border"),
            actively_inside_border: b("placement", "actively_move_inside_world_border"),
            ignored_damage: texts(&v["placement"]["cancel_creation_for_damage_types"]),
            ignored_attackers: texts(&v["placement"]["cancel_creation_for_ignored_attacker_types"]),
            blacklisted_worlds: strings(&v["placement"]["blacklisted_worlds"]).into_iter().collect(),
            blacklisted_areas: areas,
            failure_text: w("placement", "creation_default_failure_text"),
            claim_failure_text: w("placement", "creation_claim_failure_text"),
            tp_cost: Cost::of(&v["teleportation"]["cost"]),
            tp_time: i("teleportation", "required_time"),
            tp_height: v["teleportation"]["y_offset"].as_f64().unwrap_or(1.0),
            tp_moving: b("teleportation", "allow_movement_while_waiting"),
            tp_timer: Wrapped::of(v["teleportation"]["text"]["timer"].as_str().unwrap_or("")),
            tp_timer_moving: Wrapped::of(v["teleportation"]["text"]["timer_allow_moving"].as_str().unwrap_or("")),
            tp_location: Wrapped::of(v["teleportation"]["text"]["location"].as_str().unwrap_or("")),
            tp_canceled: Wrapped::of(v["teleportation"]["text"]["canceled"].as_str().unwrap_or("")),
            title: w("ui", "title"),
            admin_title: w("ui", "admin_title"),
            list_icon: Variant::of(&ui["list_grave_icon"]),
            admin_list_icon: Variant::of(&ui["admin_list_grave_icon"]),
            info_icon: Variant::of(&ui["grave_info"]),
            unlock: Variant::of(&ui["unlock_grave"]),
            previous: Variant::of(&ui["previous_button"]),
            next: Variant::of(&ui["next_button"]),
            remove_protection: Variant::of(&ui["remove_protection_button"]),
            break_grave: Variant::of(&ui["break_grave_button"]),
            quick_pickup: Icon::of(&ui["quick_pickup_button"]),
            fetch: Variant::of(&ui["fetch_button"]),
            teleport: Variant::of(&ui["teleport_button"]),
            back: Icon::of(&ui["back_button"]),
            bar: Icon::of(&ui["bar"]),
            created: w("text", "grave_created"),
            protection_ended: w("text", "protection_ended"),
            expired: w("text", "grave_expired"),
            broken: w("text", "grave_broken"),
            no_access: w("text", "grave_access_payment_no_access"),
            accepted: w("text", "grave_payment_accepted"),
            payment_failed: w("text", "grave_payment_failed"),
            suffixes: [suffix("years_suffix"), suffix("days_suffix"), suffix("hours_suffix"), suffix("minutes_suffix"), suffix("seconds_suffix")],
            infinity: suffix("infinity"),
            date_format: suffix("date_format"),
            world_names: texts(&t["world_names"]),
        }
    }

    /// `getFormattedTime`.
    fn time(&self, time: i64) -> String {
        let [y, d, h, m, s] = &self.suffixes;
        if time < 0 {
            return format!("0{s}");
        }
        let mut out = String::new();
        let (secs, mins, hours, days, years) =
            (time % 60, (time / 60) % 60, (time / 3600) % 24, time / 86400 % 365, time / (86400 * 365));
        if years > 0 {
            out.push_str(&format!("{years}{y}"));
        }
        if days > 0 {
            out.push_str(&format!("{days}{d}"));
        }
        if hours > 0 {
            out.push_str(&format!("{hours}{h}"));
        }
        if mins > 0 {
            out.push_str(&format!("{mins}{m}"));
        }
        if secs > 0 || time <= 0 {
            out.push_str(&format!("{secs}{s}"));
        }
        out
    }
}

thread_local! {
    static CONFIG: RefCell<Option<std::rc::Rc<Config>>> = const { RefCell::new(None) };
}

fn config_path() -> Option<PathBuf> {
    Game::native_config_dir().ok().map(|d| d.join("universal-graves"))
}

/// `ConfigManager.loadConfig`: the file, written with the defaults when it
/// is missing; what it leaves out keeps its default. False when it can't be read.
fn load_config() -> bool {
    let mut merged: Value = serde_json::from_str(DEFAULT_CONFIG).unwrap_or(Value::Null);
    let mut ok = true;
    if let Some(dir) = config_path() {
        let _ = std::fs::create_dir_all(dir.join("models"));
        let file = dir.join("config.json");
        match std::fs::read_to_string(&file) {
            Ok(s) => match serde_json::from_str::<Value>(&s) {
                Ok(user) => merge(&mut merged, &user),
                Err(_) => ok = false,
            },
            Err(_) => {
                let _ = std::fs::write(&file, DEFAULT_CONFIG);
            }
        }
    }
    let config = Config::from(&merged);
    CONFIG.with(|c| *c.borrow_mut() = Some(std::rc::Rc::new(config)));
    ok
}

fn cfg() -> std::rc::Rc<Config> {
    if CONFIG.with(|c| c.borrow().is_none()) {
        load_config();
    }
    CONFIG.with(|c| c.borrow().clone().unwrap())
}

// ---- the graves (`Grave`, `PositionedItemStack`) ------------------------------------------

/// Which container an item came from (`GraveInventoryMask`).
#[derive(Clone, Debug, PartialEq)]
enum Mask {
    /// `universal_graves:vanilla`: the inventory, by `Inventory` slot.
    Vanilla,
    /// `universal_graves:accessories`: an Accessories slot (`Slot`) and
    /// whether the cosmetic one (`Type`).
    Accessories(String, bool),
}

#[derive(Clone, Debug)]
struct Stored {
    stack: ItemStack,
    slot: i32,
    mask: Mask,
    tags: Vec<String>,
}

#[derive(Clone, Debug)]
struct Profile {
    uuid: u128,
    name: String,
    /// The `textures` property: value and signature.
    textures: Option<(String, Option<String>)>,
}

#[derive(Clone, Debug)]
struct Grave {
    id: i64,
    owner: Option<Profile>,
    xp: i32,
    creation_time: i64,
    game_creation_time: i64,
    death_cause: Value,
    allowed: Vec<u128>,
    items: Vec<Stored>,
    dimension: String,
    pos: [i32; 3],
    removed: bool,
    protection_enabled: bool,
    require_payment: bool,
    protection_message_sent: bool,
    minecraft_day: i32,
    skin_parts: u8,
    main_arm: u8,
    /// An emptied grave left as a gravestone (`VisualGraveBlock`).
    visual: bool,
    /// What the grave replaced: its state id and its name (`replacedBlockState`).
    replaced: Option<(u32, String)>,
    /// The block rotation's yaw (`ROTATION_16`).
    yaw: f32,
    model: String,
    /// The model's entities this session: their id and what they show.
    shown: Vec<(u32, String)>,
    /// Whether the model's entities saved with the world are cleared (after a load).
    cleaned: bool,
}

impl Grave {
    fn item_count(&self) -> usize {
        self.items.iter().filter(|s| s.stack.count > 0).count()
    }
    fn is_empty(&self) -> bool {
        if cfg().only_xp && self.xp != 0 {
            return false;
        }
        self.item_count() == 0
    }
    fn time_left(&self, duration: i32, state: &State) -> i64 {
        if cfg().use_real_time {
            self.creation_time + i64::from(duration) - now_secs()
        } else {
            self.game_creation_time + i64::from(duration) - state.game_time
        }
    }
    /// `isTimeProtected`.
    fn time_protected(&self, state: &State) -> bool {
        let time = state.protection_time;
        if time > -1 && time != 0 {
            self.time_left(time, state) > 0
        } else {
            time != 0
        }
    }
    fn is_protected(&self, state: &State) -> bool {
        !self.visual && self.time_protected(state) && self.protection_enabled
    }
    fn should_break(&self, state: &State) -> bool {
        let time = state.breaking_time;
        !self.visual && time > -1 && self.time_left(time, state) <= 0
    }
    fn is_owner(&self, uuid: u128) -> bool {
        self.owner.as_ref().is_some_and(|o| o.uuid == uuid)
    }
    fn has_access(&self, uuid: u128, state: &State) -> bool {
        !self.is_protected(state) || self.is_owner(uuid) || self.allowed.contains(&uuid)
    }
    fn owner_name(&self) -> String {
        self.owner.as_ref().map_or_else(|| "<No player!>".to_owned(), |o| o.name.clone())
    }
}

/// `Grave.getPlaceholders`.
fn placeholders(g: &Grave, state: &State, lang: &str) -> HashMap<&'static str, Value> {
    let c = cfg();
    let protection = if state.protection_time > -1 { c.time(g.time_left(state.protection_time, state)) } else { c.infinity.clone() };
    let breaking = if state.breaking_time > -1 { c.time(g.time_left(state.breaking_time, state)) } else { c.infinity.clone() };
    let mut m = HashMap::new();
    m.insert("player", text::literal(&g.owner_name()));
    m.insert("protection_time", text::literal(&protection));
    m.insert("break_time", text::literal(&breaking));
    m.insert("xp", text::literal(&g.xp.to_string()));
    m.insert("item_count", text::literal(&g.item_count().to_string()));
    m.insert("position", text::literal(&format!("{}, {}, {}", g.pos[0], g.pos[1], g.pos[2])));
    m.insert("world", world_name(&g.dimension, lang));
    m.insert("death_cause", g.death_cause.clone());
    m.insert("minecraft_day", text::literal(&g.minecraft_day.to_string()));
    m.insert("creation_date", text::literal(&format_date(&c.date_format, g.creation_time)));
    m.insert("since_creation", text::literal(&c.time(now_secs() - g.creation_time)));
    m.insert("id", text::literal(&g.id.to_string()));
    m.insert("cost", c.unlock_cost.text(lang));
    m
}

/// `GraveUtils.toWorldName`: the override, else the path's words capitalised.
fn world_name(dimension: &str, lang: &str) -> Value {
    if let Some(w) = cfg().world_names.get(dimension) {
        return w.text(lang);
    }
    let path = dimension.split_once(':').map_or(dimension, |(_, p)| p);
    let words: Vec<String> = path
        .split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or_else(String::new, |f| f.to_uppercase().collect::<String>() + c.as_str())
        })
        .collect();
    text::literal(&words.join(" "))
}

/// A Java `SimpleDateFormat` pattern (`dd.MM.yyyy, HH:mm`) in the local time.
fn format_date(pattern: &str, secs: i64) -> String {
    use chrono::TimeZone;
    let Some(t) = chrono::Local.timestamp_opt(secs, 0).single() else {
        return String::new();
    };
    let mut fmt = String::new();
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let mut n = 1;
        while i + n < chars.len() && chars[i + n] == c {
            n += 1;
        }
        let spec = match (c, n) {
            ('y', 2) => "%y",
            ('y', _) => "%Y",
            ('M', 1 | 2) => "%m",
            ('M', 3) => "%b",
            ('M', _) => "%B",
            ('d', _) => "%d",
            ('H', _) => "%H",
            ('h', _) => "%I",
            ('m', _) => "%M",
            ('s', _) => "%S",
            ('a', _) => "%p",
            ('%', _) => "%%",
            _ => "",
        };
        if spec.is_empty() {
            for _ in 0..n {
                fmt.push(c);
            }
        } else {
            fmt.push_str(spec);
        }
        i += n;
    }
    t.format(&fmt).to_string()
}

fn map_lookup(map: HashMap<&'static str, Value>) -> impl Fn(&str) -> Option<Value> {
    move |k| map.get(k).cloned()
}

// ---- the state ----------------------------------------------------------------------------

/// A player dying, heard at the start of the game's `guest_die`.
pub(super) struct Dying {
    pub uuid: u128,
    pub name: String,
    pub textures: Option<(String, Option<String>)>,
    pub xp_level: i32,
    pub xp_progress: f32,
    pub dimension: String,
    pub position: [f64; 3],
    pub damage_type: String,
    pub attacker: u32,
    pub skin_parts: u8,
    pub main_arm: u8,
    pub language: String,
    pub death_cause: Option<Value>,
}

/// A grave filled at a death, placed at the next tick of its level.
struct Pending {
    grave: Grave,
    language: String,
    take_accessories: bool,
}

/// What players asked for in a hook, done at the tick with the hub in hand.
enum Action {
    Use { uuid: u128, dimension: String, pos: [i32; 3], sneaking: bool },
    Break { uuid: u128, dimension: String, pos: [i32; 3] },
    Compass { uuid: u128, grave: i64 },
    Click { uuid: u128, window: i32, slot: i32 },
    Closed { uuid: u128, window: i32 },
    Command { uuid: u128, permission: u8, args: Vec<String> },
}

/// An open menu (`GraveGui`, `GraveListGui`, `AllGraveListGui`).
#[derive(Clone)]
enum Kind {
    List { target: u128, name: String },
    All,
    Grave { id: i64, previous: Option<Box<Kind>> },
}

#[derive(Clone, Copy)]
enum Click {
    Item(usize),
    Open(i64),
    Previous,
    Next,
    Back,
    Unlock,
    RemoveProtection,
    BreakGrave,
    QuickPickup,
    Teleport,
    Fetch,
}

struct Gui {
    kind: Kind,
    window: i32,
    page: usize,
    can_modify: bool,
    can_fetch: bool,
    ticker: u32,
    confirm: Option<u32>,
    confirm_fetch: Option<u32>,
    clicks: Vec<Option<Click>>,
}

/// A teleport to a grave counting down (`teleportToGrave`).
struct Teleport {
    uuid: u128,
    dimension: String,
    to: [f64; 3],
    from: [f64; 3],
    ticks: i32,
    refund: bool,
}

#[derive(Default)]
struct State {
    world: Option<PathBuf>,
    graves: Vec<Grave>,
    next_id: i64,
    game_time: i64,
    ticker: u32,
    last_tick: Option<i64>,
    dirty: bool,
    protection_time: i32,
    breaking_time: i32,
    dying: Option<Dying>,
    cancel_orbs: bool,
    pending: Vec<Pending>,
    messages: Vec<(u128, Value)>,
    actions: Vec<Action>,
    guis: HashMap<u128, Gui>,
    dim_second: HashMap<String, i64>,
    /// Graves gone while their chunk was away, whose model entities go when it is back.
    orphans: Vec<(String, [i32; 3], i64)>,
    teleports: Vec<Teleport>,
    respawned: Vec<u128>,
    /// Accessories' slot drop rules (`dropRule`), by slot name.
    accessory_rules: HashMap<String, String>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

// ---- saving (`GraveManager` as `SavedData`) ----------------------------------------------

fn data_file(world: &std::path::Path) -> PathBuf {
    world.join("data").join("universal_graves").join("graves.dat")
}

fn stored_nbt(s: &Stored) -> Tag {
    let mut c = Compound::new();
    c.insert(jkey("Item"), horizon_items::item_nbt::item_to_nbt(&s.stack));
    c.insert(jkey("Slot"), Tag::Int(s.slot));
    match &s.mask {
        Mask::Vanilla => {
            c.insert(jkey("Mask"), sstr("universal_graves:vanilla"));
        }
        Mask::Accessories(slot, cosmetic) => {
            c.insert(jkey("Mask"), sstr("universal_graves:accessories"));
            let mut d = Compound::new();
            d.insert(jkey("Type"), sstr(if *cosmetic { "cosmetic" } else { "" }));
            d.insert(jkey("Slot"), sstr(slot));
            c.insert(jkey("Data"), Tag::Compound(d));
        }
    }
    if !s.tags.is_empty() {
        c.insert(jkey("Tags"), Tag::List(s.tags.iter().map(|t| sstr(t)).collect()));
    }
    Tag::Compound(c)
}

fn stored_of(c: &Compound) -> Option<Stored> {
    let stack = horizon_items::item_nbt::item_from_nbt(c.get(&jkey("Item"))?)?;
    let mask = match get_str(c, "Mask").as_deref() {
        Some("universal_graves:accessories") => {
            let d = match c.get(&jkey("Data")) {
                Some(Tag::Compound(d)) => d.clone(),
                _ => Compound::new(),
            };
            Mask::Accessories(get_str(&d, "Slot").unwrap_or_default(), get_str(&d, "Type").as_deref() == Some("cosmetic"))
        }
        _ => Mask::Vanilla,
    };
    let tags = match c.get(&jkey("Tags")) {
        Some(Tag::List(l)) => l.iter().filter_map(|t| if let Tag::String(s) = t { Some(jstr(s)) } else { None }).collect(),
        _ => Vec::new(),
    };
    Some(Stored { stack, slot: get_i64(c, "Slot").unwrap_or(-1) as i32, mask, tags })
}

/// `Grave.writeNbt`, with the port's own keys (`Horizon*`) for what the
/// original keeps in its block entity.
fn grave_nbt(g: &Grave) -> Tag {
    let mut c = Compound::new();
    if let Some(o) = &g.owner {
        let mut p = Compound::new();
        p.insert(jkey("Name"), sstr(&o.name));
        p.insert(jkey("Id"), Tag::IntArray(uuid_ints(o.uuid)));
        if let Some((value, signature)) = &o.textures {
            let mut prop = Compound::new();
            prop.insert(jkey("Value"), sstr(value));
            if let Some(s) = signature {
                prop.insert(jkey("Signature"), sstr(s));
            }
            let mut props = Compound::new();
            props.insert(jkey("textures"), Tag::List(vec![Tag::Compound(prop)]));
            p.insert(jkey("Properties"), Tag::Compound(props));
        }
        c.insert(jkey("GameProfile"), Tag::Compound(p));
    }
    c.insert(jkey("Id"), Tag::Long(g.id));
    c.insert(jkey("XP"), Tag::Int(g.xp));
    c.insert(jkey("CreationTime"), Tag::Long(g.creation_time));
    c.insert(jkey("TickCreationTime"), Tag::Long(g.game_creation_time));
    c.insert(jkey("ItemCount"), Tag::Int(g.item_count() as i32));
    c.insert(jkey("MinecraftDay"), Tag::Int(g.minecraft_day));
    c.insert(jkey("DeathCause"), sstr(&g.death_cause.to_string()));
    c.insert(jkey("Type"), sstr(if g.visual { "VISUAL" } else { "BLOCK" }));
    c.insert(jkey("IsProtectionEnabled"), Tag::Byte(i8::from(g.protection_enabled)));
    c.insert(jkey("RequirePayment"), Tag::Byte(i8::from(g.require_payment)));
    c.insert(jkey("Position"), Tag::IntArray(g.pos.to_vec()));
    c.insert(jkey("World"), sstr(&g.dimension));
    c.insert(jkey("AllowedUUIDs"), Tag::List(g.allowed.iter().map(|u| Tag::IntArray(uuid_ints(*u))).collect()));
    c.insert(jkey("Items"), Tag::List(g.items.iter().map(stored_nbt).collect()));
    c.insert(jkey("SkinModelParts"), Tag::Byte(g.skin_parts as i8));
    c.insert(jkey("MainArm"), Tag::Byte(g.main_arm as i8));
    if let Some((id, name)) = &g.replaced {
        c.insert(jkey("HorizonReplacedState"), Tag::Int(*id as i32));
        c.insert(jkey("HorizonReplacedBlock"), sstr(name));
    }
    c.insert(jkey("HorizonYaw"), Tag::Float(g.yaw.to_bits()));
    c.insert(jkey("HorizonModel"), sstr(&g.model));
    Tag::Compound(c)
}

fn grave_of(c: &Compound) -> Option<Grave> {
    let owner = match c.get(&jkey("GameProfile")) {
        Some(Tag::Compound(p)) => {
            let uuid = match p.get(&jkey("Id")) {
                Some(Tag::IntArray(v)) => ints_uuid(v),
                _ => None,
            };
            let textures = match p.get(&jkey("Properties")) {
                Some(Tag::Compound(props)) => match props.get(&jkey("textures")) {
                    Some(Tag::List(l)) => l.iter().find_map(|t| match t {
                        Tag::Compound(t) => get_str(t, "Value").map(|v| (v, get_str(t, "Signature"))),
                        _ => None,
                    }),
                    _ => None,
                },
                _ => None,
            };
            uuid.map(|uuid| Profile { uuid, name: get_str(p, "Name").unwrap_or_default(), textures })
        }
        _ => None,
    };
    let pos = match c.get(&jkey("Position")) {
        Some(Tag::IntArray(v)) if v.len() == 3 => [v[0], v[1], v[2]],
        _ => return None,
    };
    let items = match c.get(&jkey("Items")) {
        Some(Tag::List(l)) => l.iter().filter_map(|t| if let Tag::Compound(t) = t { stored_of(t) } else { None }).collect(),
        _ => Vec::new(),
    };
    let allowed = match c.get(&jkey("AllowedUUIDs")) {
        Some(Tag::List(l)) => l.iter().filter_map(|t| if let Tag::IntArray(v) = t { ints_uuid(v) } else { None }).collect(),
        _ => Vec::new(),
    };
    let replaced = match (get_i64(c, "HorizonReplacedState"), get_str(c, "HorizonReplacedBlock")) {
        (Some(id), Some(name)) => Some((id as u32, name)),
        _ => None,
    };
    let protection_enabled = get_bool(c, "IsProtectionEnabled").unwrap_or(true);
    Some(Grave {
        id: get_i64(c, "Id").unwrap_or(-1),
        owner,
        xp: get_i64(c, "XP").unwrap_or(0) as i32,
        creation_time: get_i64(c, "CreationTime").unwrap_or(0),
        game_creation_time: get_i64(c, "TickCreationTime").unwrap_or(0),
        death_cause: get_str(c, "DeathCause").and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| text::literal("Unknown cause")),
        allowed,
        items,
        dimension: get_str(c, "World").unwrap_or_else(|| "minecraft:overworld".into()),
        pos,
        removed: false,
        protection_enabled,
        require_payment: get_bool(c, "RequirePayment").unwrap_or(false),
        protection_message_sent: false,
        minecraft_day: get_i64(c, "MinecraftDay").unwrap_or(0) as i32,
        skin_parts: get_i64(c, "SkinModelParts").unwrap_or(0xFF) as u8,
        main_arm: get_i64(c, "MainArm").unwrap_or(1) as u8,
        visual: get_str(c, "Type").as_deref() == Some("VISUAL"),
        replaced,
        yaw: match c.get(&jkey("HorizonYaw")) {
            Some(Tag::Float(f)) => f32::from_bits(*f),
            _ => 0.0,
        },
        model: get_str(c, "HorizonModel").unwrap_or_else(|| "default".into()),
        shown: Vec::new(),
        cleaned: false,
    })
}

fn load_graves(state: &mut State, world: &std::path::Path) {
    state.graves.clear();
    state.next_id = 0;
    state.game_time = 0;
    let Ok(bytes) = std::fs::read(data_file(world)) else {
        return;
    };
    let mut raw = Vec::new();
    if std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(&bytes[..]), &mut raw).is_err() {
        return;
    }
    let Ok((root, _)) = horizon_world::nbt::read_named(&raw, horizon_world::nbt::Limits::default()) else {
        return;
    };
    let Tag::Compound(root) = root.tag else {
        return;
    };
    let data = match root.get(&jkey("data")) {
        Some(Tag::Compound(d)) => d.clone(),
        _ => root,
    };
    state.game_time = get_i64(&data, "CurrentGameTime").unwrap_or(0);
    state.next_id = get_i64(&data, "CurrentGrave").unwrap_or(0);
    if let Some(Tag::List(l)) = data.get(&jkey("Graves")) {
        for t in l {
            if let Tag::Compound(c) = t
                && let Some(mut g) = grave_of(c)
            {
                if g.id < 0 {
                    g.id = state.next_id;
                    state.next_id += 1;
                }
                g.protection_message_sent = !g.is_protected(state);
                state.graves.push(g);
            }
        }
    }
    if let Some(Tag::List(l)) = data.get(&jkey("HorizonOrphans")) {
        for t in l {
            if let Tag::Compound(c) = t
                && let (Some(d), Some(Tag::IntArray(p)), Some(id)) = (get_str(c, "World"), c.get(&jkey("Position")), get_i64(c, "Grave"))
                && p.len() == 3
            {
                state.orphans.push((d, [p[0], p[1], p[2]], id));
            }
        }
    }
}

fn save_graves(state: &mut State) {
    let Some(world) = state.world.clone() else {
        return;
    };
    let mut data = Compound::new();
    let graves: Vec<Tag> = state.graves.iter().filter(|g| !g.removed).map(grave_nbt).collect();
    data.insert(jkey("Graves"), Tag::List(graves));
    data.insert(jkey("Version"), Tag::Int(3));
    data.insert(jkey("CurrentGameTime"), Tag::Long(state.game_time));
    data.insert(jkey("CurrentGrave"), Tag::Long(state.next_id));
    data.insert(
        jkey("HorizonOrphans"),
        Tag::List(
            state
                .orphans
                .iter()
                .map(|(d, p, id)| {
                    let mut c = Compound::new();
                    c.insert(jkey("World"), sstr(d));
                    c.insert(jkey("Position"), Tag::IntArray(p.to_vec()));
                    c.insert(jkey("Grave"), Tag::Long(*id));
                    Tag::Compound(c)
                })
                .collect(),
        ),
    );
    let mut root = Compound::new();
    root.insert(jkey("data"), Tag::Compound(data));
    let named = horizon_world::nbt::NamedTag { name: jkey(""), tag: Tag::Compound(root) };
    let Ok(raw) = horizon_world::nbt::write_named(&named, horizon_world::nbt::Limits::default()) else {
        return;
    };
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    if std::io::Write::write_all(&mut gz, &raw).is_err() {
        return;
    }
    let Ok(bytes) = gz.finish() else {
        return;
    };
    let file = data_file(&world);
    if let Some(dir) = file.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = file.with_extension("dat_new");
    if std::fs::write(&tmp, bytes).is_ok() {
        let _ = std::fs::rename(&tmp, &file);
    }
    state.dirty = false;
}

/// The world was saved: the graves are too.
pub(super) fn world_saved() {
    with(|s| {
        if s.world.is_some() {
            save_graves(s);
        }
    });
}

// ---- players ------------------------------------------------------------------------------

fn guest_index(game: &Game<'_>, uuid: u128) -> Option<usize> {
    game.lan.as_ref()?.hub.guests.iter().position(|g| g.placed && g.uuid == uuid)
}

fn language(game: &Game<'_>, uuid: u128) -> String {
    game.lan
        .as_ref()
        .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid))
        .map_or_else(|| "en_us".to_owned(), |g| g.info.language.to_ascii_lowercase())
}

fn tell(game: &mut Game<'_>, uuid: u128, v: Value) {
    let _ = game.native_tell(uuid, to_text(&v));
}

fn sound(game: &mut Game<'_>, uuid: u128, sound: &str, volume: f32, pitch: f32) {
    let _ = game.native_play_sound_to(uuid, sound, "master", None, volume, pitch);
}

/// `GuiSlot`'s click sound (`playClickSound`).
fn click_sound(game: &mut Game<'_>, uuid: u128) {
    sound(game, uuid, "minecraft:ui.button.click", 0.5, 1.0);
}

fn player_data(game: &Game<'_>, uuid: u128) -> Compound {
    match game.native_player_persistent_data(uuid) {
        Some(Tag::Compound(c)) => match c.get(&jkey(PLAYER_KEY)) {
            Some(Tag::Compound(d)) => d.clone(),
            _ => Compound::new(),
        },
        _ => Compound::new(),
    }
}

fn set_player_data(game: &mut Game<'_>, uuid: u128, d: Compound) {
    let mut all = match game.native_player_persistent_data(uuid) {
        Some(Tag::Compound(c)) => c,
        _ => Compound::new(),
    };
    all.insert(jkey(PLAYER_KEY), Tag::Compound(d));
    let _ = game.native_set_player_persistent_data(uuid, Tag::Compound(all));
}

/// A stack into a player's inventory (`placeItemBackInInventory`); what has
/// no room drops at their feet.
fn give(game: &mut Game<'_>, uuid: u128, stack: ItemStack) {
    let max = game.items().get(&stack.item).map_or(64, |i| i.max_stack);
    let mut stack = stack;
    let left = match guest_index(game, uuid) {
        Some(gi) => {
            let host = game.lan.as_mut().unwrap();
            let g = &mut host.hub.guests[gi];
            let left = super::inventory_add(&mut g.inventory, &mut g.offhand, g.selected, max, &mut stack);
            let at = g.position;
            host.hub.send_inventory(gi);
            left.then_some(at)
        }
        None => None,
    };
    if let Some(at) = left {
        game.spawn_item(stack, at, [0.0; 3], 40);
    }
}

// ---- deaths (`LivingEntityMixin`, `GraveUtils.createGrave`) --------------------------------

/// The start of `guest_die`: who is dying, with what they have.
pub(super) fn dying(d: Dying) {
    with(|s| {
        s.dying = Some(d);
        s.cancel_orbs = false;
    });
}

/// The death message, as the game made it (`getLocalizedDeathMessage`).
pub(super) fn death_message(player: u128, message: &Text) {
    with(|s| {
        if let Some(d) = s.dying.as_mut().filter(|d| d.uuid == player) {
            d.death_cause = Some(message.component());
        }
    });
}

/// `PreventEquipmentDrop`, and the config's and the tag's blocked enchantments.
fn can_add(stack: &ItemStack) -> bool {
    stack.count > 0
        && &*stack.item != "minecraft:air"
        && stack.enchantment_level("minecraft:vanishing_curse") == 0
        && !cfg().blocked_enchantments.iter().any(|e| stack.enchantment_level(e) > 0)
}

/// The result of a place check (`BlockResult`): its priority and whether a grave may go there.
#[derive(Clone, Copy, PartialEq)]
enum Spot {
    Block,
    BlockClaim,
    BlockFluid,
    Allow,
}

impl Spot {
    fn priority(self) -> i32 {
        match self {
            Spot::Block => 0,
            Spot::BlockFluid => 1,
            Spot::BlockClaim => 2,
            Spot::Allow => 3,
        }
    }
}

fn is_fluid(game: &Game<'_>, pos: [i32; 3]) -> bool {
    let name = game.native_block(pos);
    name == "minecraft:water"
        || name == "minecraft:lava"
        || name == "minecraft:bubble_column"
        || game.native_block_property(pos, "waterlogged").as_deref() == Some("true")
}

/// `canReplaceState`: air or the `universal_graves:replaceable` tag
/// (fire, flowers, snow, grass, water, lava...), never another grave.
fn can_replace(game: &Game<'_>, pos: [i32; 3], any: bool) -> bool {
    let name = game.native_block(pos);
    if name == GRAVE_BLOCK && with(|s| s.graves.iter().any(|g| !g.removed && g.pos == pos)) {
        return false;
    }
    if name.ends_with(":air") {
        return true;
    }
    if game.world.block_entities.get(&(pos[0], pos[1], pos[2])).is_some() {
        return false;
    }
    any || matches!(
        name.as_str(),
        "minecraft:snow"
            | "minecraft:short_grass"
            | "minecraft:tall_grass"
            | "minecraft:seagrass"
            | "minecraft:glow_lichen"
            | "minecraft:moss_carpet"
            | "minecraft:large_fern"
            | "minecraft:fern"
            | "minecraft:lava"
            | "minecraft:water"
    ) || game.items().block_in_tag("minecraft:fire", &name)
        || game.items().block_in_tag("minecraft:flowers", &name)
}

/// `isValidPos`.
fn valid(game: &Game<'_>, dimension: &str, pos: [i32; 3], any: bool) -> Spot {
    let c = cfg();
    let min = game.world.shape.min_y;
    let max = game.world.shape.max_y();
    let inside = !c.inside_border
        || game.native_world_border(dimension).is_none_or(|b| {
            f64::from(pos[0]) >= b[0] && f64::from(pos[0]) < b[2] && f64::from(pos[2]) >= b[1] && f64::from(pos[2]) < b[3]
        });
    if !(can_replace(game, pos, any) && inside && pos[1] >= min && pos[1] < max + 1) {
        return Spot::Block;
    }
    if c.on_fluids && is_fluid(game, pos) && is_fluid(game, [pos[0], pos[1] + 1, pos[2]]) {
        return Spot::BlockFluid;
    }
    if let Some(areas) = c.blacklisted_areas.get(dimension) {
        for a in areas {
            if a[0] < pos[0] && a[3] > pos[0] && a[1] < pos[1] && a[4] > pos[1] && a[2] < pos[2] && a[5] > pos[2] {
                return Spot::BlockClaim;
            }
        }
    }
    Spot::Allow
}

/// `findPos`: the closest valid spot, in growing cube shells, then shifted.
fn find_pos(game: &Game<'_>, dimension: &str, at: [i32; 3], any: bool, iteration: i32, seed: &mut u64) -> (Option<[i32; 3]>, Spot) {
    let c = cfg();
    let mut result = valid(game, dimension, at, any);
    if result == Spot::Allow {
        return (Some(at), result);
    }
    for line in 1..=c.max_distance.max(0) {
        let side = line * 2 + 1;
        for oy in 0..side {
            for ox in 0..side {
                for oz in 0..side {
                    if ox > 0 && ox < side - 1 && oy > 0 && oy < side - 1 && oz > 0 && oz < side - 1 {
                        continue;
                    }
                    let pos = [at[0] - line + ox, at[1] - line + oy, at[2] - line + oz];
                    let t = valid(game, dimension, pos, any);
                    if t.priority() >= result.priority() {
                        result = t;
                    }
                    if result == Spot::Allow {
                        return (Some(pos), result);
                    }
                }
            }
        }
    }
    if c.shift_on_failure && iteration < c.max_shifts {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let d = c.shift_distance;
        let shifted = match (*seed >> 33) % 6 {
            0 => [at[0], at[1] - d, at[2]],
            1 => [at[0], at[1] + d, at[2]],
            2 => [at[0], at[1], at[2] - d],
            3 => [at[0], at[1], at[2] + d],
            4 => [at[0] - d, at[1], at[2]],
            _ => [at[0] + d, at[1], at[2]],
        };
        return find_pos(game, dimension, shifted, any, iteration + 1, seed);
    }
    (None, result)
}

/// `findGravePosition`.
fn find_grave_position(game: &Game<'_>, dimension: &str, at: [i32; 3]) -> (Option<[i32; 3]>, Spot) {
    let c = cfg();
    let min = game.world.shape.min_y;
    let max = game.world.shape.max_y();
    let mut pos = at;
    match game.native_world_border(dimension).filter(|_| c.inside_border) {
        Some(b) => {
            pos[0] = pos[0].clamp(b[0] as i32 + 1, b[2] as i32 - 1);
            pos[2] = pos[2].clamp(b[1] as i32 + 1, b[3] as i32 - 1);
        }
        None => {}
    }
    pos[1] = pos[1].clamp(min, max);
    if c.on_ground {
        while game.native_block(pos).ends_with(":air") && min + 2 < pos[1] {
            pos[1] -= 1;
        }
    }
    let result = valid(game, dimension, pos, false);
    if result == Spot::Allow {
        return (Some(pos), result);
    }
    if result == Spot::BlockFluid {
        let mut p = pos;
        while is_fluid(game, p) && p[1] < max {
            p[1] += 1;
        }
        if valid(game, dimension, p, false) == Spot::Allow {
            return (Some(p), Spot::Allow);
        }
        pos = p;
    }
    let mut seed = (pos[0] as u64) ^ ((pos[2] as u64) << 21) ^ (game.world.game_time as u64);
    let found = find_pos(game, dimension, pos, false, 0, &mut seed);
    if found.0.is_none() && c.replace_any {
        return find_pos(game, dimension, pos, true, 0, &mut seed);
    }
    found
}

/// `GravesXPCalculation`'s points of `level` levels and `progress`.
fn xp_points(levels: f64, progress: f64, level: i32) -> f64 {
    let mut points = 0.0;
    let mut i = 0;
    while f64::from(i) < levels {
        points += f64::from(if i >= 30 {
            112 + (i - 30) * 9
        } else if i >= 15 {
            37 + (i - 15) * 5
        } else {
            7 + i * 2
        });
        i += 1;
    }
    let needed = if level >= 30 {
        112 + (level - 30) * 9
    } else if level >= 15 {
        37 + (level - 15) * 5
    } else {
        7 + level * 2
    };
    points + progress * f64::from(needed)
}

fn stored_xp(d: &Dying) -> i32 {
    let c = cfg();
    match c.xp_type.as_str() {
        "vanilla" | "drop" => (d.xp_level * 7).min(100),
        "percent_points" => (xp_points(f64::from(d.xp_level), f64::from(d.xp_progress), d.xp_level) * c.xp_percent / 100.0) as i32,
        "percent_levels" => {
            let p = c.xp_percent / 100.0;
            xp_points(f64::from(d.xp_level) * p, f64::from(d.xp_progress) * p, d.xp_level) as i32
        }
        _ => 0,
    }
}

/// The start of `drop_inventory` for a dying player (their state swapped
/// in): the grave takes what would drop, and is placed at the tick.
pub(super) fn drop_inventory(game: &mut Game<'_>, who: u128) {
    let Some(d) = with(|s| s.dying.take()).filter(|d| d.uuid == who) else {
        return;
    };
    let c = cfg();
    if game.ui.rules.bool("keep_inventory") || c.blacklisted_worlds.contains(&d.dimension) || c.grave_limit == 0 {
        return;
    }
    let at = d.position.map(|v| v.floor() as i32);
    let lang = d.language.clone();
    let base = {
        let position = format!("{}, {}, {}", at[0], at[1], at[2]);
        let world = world_name(&d.dimension, &lang);
        move |k: &str| match k {
            "position" => Some(text::literal(&position)),
            "world" => Some(world.clone()),
            _ => None,
        }
    };
    // `cancel_creation_for_ignored_attacker_types`, `..._damage_types`.
    let attacker_kind = (d.attacker != 0)
        .then(|| game.native_entity(d.attacker).and_then(|e| game.native_entity_kind(e)))
        .flatten();
    let refused = attacker_kind
        .as_ref()
        .and_then(|k| c.ignored_attackers.get(k))
        .or_else(|| c.ignored_damage.get(&d.damage_type))
        .cloned();
    if let Some(w) = refused {
        if !w.is_empty() {
            let v = w.with(&lang, &base);
            with(|s| s.messages.push((d.uuid, v)));
        }
        return;
    }
    let (spot, result) = find_grave_position(game, &d.dimension, at);
    let Some(spot) = spot else {
        let w = if result == Spot::BlockClaim { &c.claim_failure_text } else { &c.failure_text };
        if !w.is_empty() {
            let v = w.with(&lang, &base);
            with(|s| s.messages.push((d.uuid, v)));
        }
        return;
    };
    // `VanillaInventoryMask.addToGrave`, with the tags its model reads.
    let selected = game.selected;
    let mut items = Vec::new();
    let tag_of = |slot: i32| -> Vec<String> {
        let t = match slot {
            s if s == selected as i32 => "graves:equipment_main_hand",
            40 => "graves:equipment_offhand_hand",
            36 => "graves:equipment_boots",
            37 => "graves:equipment_leggings",
            38 => "graves:equipment_chestplate",
            39 => "graves:equipment_helmet",
            _ => return Vec::new(),
        };
        vec![t.to_owned()]
    };
    let take = |place: &mut Option<ItemStack>, slot: i32, items: &mut Vec<Stored>| {
        if place.as_ref().is_some_and(can_add) {
            let stack = place.take().unwrap();
            items.push(Stored { stack, slot, mask: Mask::Vanilla, tags: tag_of(slot) });
        }
    };
    for slot in 0..game.inventory.len().min(36) {
        let mut place = game.inventory[slot].take();
        take(&mut place, slot as i32, &mut items);
        game.inventory[slot] = place;
    }
    for k in (0..4).rev() {
        let mut place = game.armor[k].take();
        take(&mut place, 39 - k as i32, &mut items);
        game.armor[k] = place;
    }
    let mut place = game.offhand.take();
    take(&mut place, 40, &mut items);
    game.offhand = place;
    let mut carried = game.carried.take();
    take(&mut carried, -1, &mut items);
    game.carried = carried;
    let xp = if c.xp_type != "drop" { stored_xp(&d) } else { 0 };
    let accessories = game.native_player_persistent_data(d.uuid).is_some();
    // `allowedUUIDs`: the attacker, when attackers bypass the protection.
    let mut allowed = Vec::new();
    if c.attackers_bypass && attacker_kind.as_deref() == Some("minecraft:player") {
        if let Some(u) = game.native_entity(d.attacker).and_then(|e| game.native_entity_uuid(e)) {
            allowed.push(u);
        }
    }
    let replaced_state = game.world.block(spot[0], spot[1], spot[2]);
    let replaced = Some((replaced_state.0, game.state_string(replaced_state)));
    let minecraft_day = (game.native_day_time() / 24000) as i32;
    let mut seed = (game.world.game_time as u64) ^ (d.uuid as u64);
    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let yaw = ((seed >> 33) % 15) as f32 * 22.5;
    let grave = Grave {
        id: -1,
        owner: Some(Profile { uuid: d.uuid, name: d.name.clone(), textures: d.textures.clone() }),
        xp,
        creation_time: now_secs(),
        game_creation_time: 0,
        death_cause: d.death_cause.clone().unwrap_or_else(|| text::literal("Unknown cause")),
        allowed,
        items,
        dimension: d.dimension.clone(),
        pos: spot,
        removed: false,
        protection_enabled: true,
        require_payment: !c.unlock_cost.is_free(),
        protection_message_sent: false,
        minecraft_day,
        skin_parts: d.skin_parts,
        main_arm: d.main_arm,
        visual: false,
        replaced,
        yaw,
        model: "default".to_owned(),
        shown: Vec::new(),
        cleaned: true,
    };
    with(|s| {
        // The vanilla experience doesn't drop: the grave keeps it (`experienceLevel = 0`).
        s.cancel_orbs = c.xp_type != "drop";
        s.pending.push(Pending { grave, language: lang, take_accessories: accessories });
    });
}

/// `ExperienceOrb.award` for a death whose grave keeps the experience: none drops.
pub(super) fn spawn_orbs() -> bool {
    with(|s| std::mem::take(&mut s.cancel_orbs))
}

/// Accessories' slots into a pending grave (the original's `AccessoriesCompat`),
/// by each slot's `dropRule`, before Accessories drops them at its own tick.
fn take_accessories(game: &mut Game<'_>, uuid: u128, items: &mut Vec<Stored>) {
    const HOLDER: &str = "accessories:inventory_holder";
    let Some(Tag::Compound(mut all)) = game.native_player_persistent_data(uuid) else {
        return;
    };
    let Some(Tag::Compound(mut holder)) = all.get(&jkey(HOLDER)).cloned() else {
        return;
    };
    let Some(Tag::Compound(mut containers)) = holder.get(&jkey("accessories_containers")).cloned() else {
        return;
    };
    let rules = with(|s| s.accessory_rules.clone());
    let mut changed = false;
    for (name, tag) in containers.iter_mut() {
        let Tag::Compound(sc) = tag else {
            continue;
        };
        let slot_name = jstr(name);
        let rule = rules.get(&slot_name).map_or("default", String::as_str).to_ascii_lowercase();
        for (key, cosmetic) in [("items", false), ("cosmetics", true)] {
            let Some(Tag::List(list)) = sc.get(&jkey(key)).cloned() else {
                continue;
            };
            let mut list = list;
            for entry in list.iter_mut() {
                let Tag::Compound(e) = entry else {
                    continue;
                };
                let Some(stack) = horizon_items::item_nbt::item_from_nbt(&Tag::Compound(e.clone())) else {
                    continue;
                };
                if stack.count == 0 || !can_add(&stack) {
                    continue;
                }
                let drop = match rule.as_str() {
                    "keep" | "destroy" => false,
                    _ => true,
                };
                if !drop {
                    continue;
                }
                let index = get_i64(e, "Slot").unwrap_or(0) as i32;
                items.push(Stored { stack, slot: index, mask: Mask::Accessories(slot_name.clone(), cosmetic), tags: Vec::new() });
                // An emptied entry: the slot stays, its item goes.
                let mut empty = Compound::new();
                empty.insert(jkey("Slot"), Tag::Byte(index as i8));
                *e = empty;
                changed = true;
            }
            sc.insert(jkey(key), Tag::List(list.into_iter().filter(|t| matches!(t, Tag::Compound(c) if c.get(&jkey("id")).is_some())).collect()));
        }
    }
    if changed {
        holder.insert(jkey("accessories_containers"), Tag::Compound(containers));
        all.insert(jkey(HOLDER), Tag::Compound(holder));
        let _ = game.native_set_player_persistent_data(uuid, Tag::Compound(all));
    }
}

/// The next tick's half of `createGrave`: the grave is placed, kept and told
/// of, or its contents drop where it would have been.
fn place_pending(game: &mut Game<'_>, p: Pending) {
    let Pending { mut grave, language: lang, take_accessories: accessories } = p;
    let uuid = grave.owner.as_ref().map_or(0, |o| o.uuid);
    if accessories {
        take_accessories(game, uuid, &mut grave.items);
    }
    if grave.items.is_empty() && (!cfg().only_xp || grave.xp == 0) {
        // Nothing to keep: the experience drops as vanilla's would have.
        if grave.xp > 0 {
            let at = grave.pos.map(|v| f64::from(v) + 0.5);
            game.spawn_orbs(at, grave.xp as u32);
        }
        return;
    }
    let pos = grave.pos;
    let state = game.native_default_state(GRAVE_BLOCK);
    let waterlogged = game.native_block(pos) == "minecraft:water";
    let placed = state.is_some_and(|st| game.native_set_block(&grave.dimension, pos, st, 3));
    if placed && waterlogged {
        let _ = game.native_set_block_property(pos, "waterlogged", "true");
    }
    if !placed {
        // `creation_default_failure_text`: the items drop at the spot.
        let at = pos.map(|v| f64::from(v) + 0.5);
        for s in std::mem::take(&mut grave.items) {
            game.spawn_item_own(s.stack, at, 10);
        }
        if grave.xp > 0 {
            game.spawn_orbs(at, grave.xp as u32);
        }
        let w = cfg().failure_text.clone();
        if !w.is_empty() {
            let position = format!("{}, {}, {}", pos[0], pos[1], pos[2]);
            let world = world_name(&grave.dimension, &lang);
            let v = w.with(&lang, &|k| match k {
                "position" => Some(text::literal(&position)),
                "world" => Some(world.clone()),
                _ => None,
            });
            tell(game, uuid, v);
        }
        return;
    }
    let (id, message, evicted) = with(|s| {
        grave.id = s.next_id;
        s.next_id += 1;
        grave.game_creation_time = s.game_time;
        grave.protection_message_sent = !grave.is_protected(s);
        let ph = placeholders(&grave, s, &lang);
        let id = grave.id;
        s.graves.push(grave);
        s.dirty = true;
        // `player_grave_limit`: the oldest graves beyond it go.
        let limit = cfg().grave_limit;
        let mut evicted = Vec::new();
        if limit > -1 {
            let mut mine: Vec<(i64, i64)> = s
                .graves
                .iter()
                .filter(|g| !g.removed && !g.visual && g.is_owner(uuid))
                .map(|g| (g.creation_time, g.id))
                .collect();
            mine.sort();
            while mine.len() > limit as usize {
                evicted.push(mine.remove(0).1);
            }
        }
        let w = cfg().created.clone();
        let message = (!w.is_empty()).then(|| w.with(&lang, &map_lookup(ph)));
        (id, message, evicted)
    });
    let mut d = player_data(game, uuid);
    d.insert(jkey("LastGraveId"), Tag::Long(id));
    d.insert(jkey("HasGraveCompass"), Tag::Byte(0));
    set_player_data(game, uuid, d);
    for e in evicted {
        destroy_grave(game, e, None);
    }
    if let Some(v) = message {
        tell(game, uuid, v);
    }
}

// ---- breaking (`Grave.destroyGrave`, `GraveBlockEntity.breakBlock`) -----------------------

fn grave_index(state: &State, id: i64) -> Option<usize> {
    state.graves.iter().position(|g| g.id == id && !g.removed)
}

fn grave_at(state: &State, dimension: &str, pos: [i32; 3]) -> Option<usize> {
    state.graves.iter().position(|g| !g.removed && g.pos == pos && g.dimension == dimension)
}

/// `destroyGrave`: the owner is told, the items and experience drop (unless it
/// expired and `drop_items_on_expiration` is off), the block goes back to what
/// it replaced. A grave in a level not in place, or a chunk not loaded, waits
/// for its level's tick.
fn destroy_grave(game: &mut Game<'_>, id: i64, breaker: Option<u128>) -> bool {
    let Some((grave, should_break)) = with(|s| grave_index(s, id).map(|i| (s.graves[i].clone(), s.graves[i].should_break(s)))) else {
        return false;
    };
    if grave.dimension != game.dim.id() || !chunk_loaded(game, &grave.dimension, grave.pos) {
        with(|s| {
            if let Some(i) = grave_index(s, id) {
                s.graves[i].removed = true;
                s.dirty = true;
                // The block and the model go when the level and chunk are back.
                s.graves[i].items.clear();
                s.graves[i].xp = 0;
            }
        });
        return true;
    }
    let c = cfg();
    let owner = grave.owner.as_ref().map(|o| o.uuid);
    if let Some(owner) = owner.filter(|o| Some(*o) != breaker && guest_index(game, *o).is_some()) {
        let w = if should_break { &c.expired } else { &c.broken };
        if !w.is_empty() && !grave.visual {
            let lang = language(game, owner);
            let v = with(|s| w.with(&lang, &map_lookup(placeholders(&grave, s, &lang))));
            tell(game, owner, v);
        }
    }
    with(|s| {
        if let Some(i) = grave_index(s, id) {
            s.graves[i].removed = true;
            s.dirty = true;
        }
    });
    let at = grave.pos.map(|v| f64::from(v) + 0.5);
    if c.drop_items_on_expiration || !should_break {
        for s in &grave.items {
            if s.stack.count > 0 {
                game.spawn_item_own(s.stack.clone(), at, 10);
            }
        }
        if grave.xp > 0 {
            game.spawn_orbs(at, grave.xp as u32);
        }
    }
    remove_model(game, &grave);
    restore_block(game, &grave);
    true
}

/// `breakBlock`: the replaced block back (or air), or a gravestone with
/// `create_gravestone_after_emptying`.
fn restore_block(game: &mut Game<'_>, grave: &Grave) {
    if game.native_block(grave.pos) != GRAVE_BLOCK {
        return;
    }
    if cfg().visual_after_emptying && !grave.visual {
        let mut v = grave.clone();
        v.visual = true;
        v.items.clear();
        v.xp = 0;
        v.shown.clear();
        with(|s| {
            v.id = s.next_id;
            s.next_id += 1;
            s.graves.push(v);
            s.dirty = true;
        });
        return;
    }
    let state = grave
        .replaced
        .as_ref()
        .and_then(|(id, name)| {
            let id = horizon_world::registry::BlockStateId(*id);
            (game.state_string(id) == *name).then_some(id).or_else(|| game.native_default_state(name.split('[').next().unwrap_or(name)))
        })
        .filter(|_| cfg().restore_after_breaking)
        .or_else(|| game.native_default_state("minecraft:air"));
    if let Some(st) = state {
        let _ = game.native_set_block(&grave.dimension, grave.pos, st, 3);
    }
}

fn chunk_loaded(game: &Game<'_>, dimension: &str, pos: [i32; 3]) -> bool {
    game.native_level(dimension).is_some_and(|l| game.native_level_is_loaded(l, pos))
}

// ---- the model (`GraveModelHandler`, `DefaultGraveModels.playerHead`) ---------------------

type M = [[f32; 4]; 4];

fn mul(a: &M, b: &M) -> M {
    let mut r = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            r[i][j] = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

fn ident() -> M {
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]
}

/// JOML's chained `Matrix4f` calls, each multiplied on the right.
struct Mat(M);

impl Mat {
    fn new() -> Self {
        Self(ident())
    }
    fn then(self, m: M) -> Self {
        Self(mul(&self.0, &m))
    }
    fn translate(self, x: f32, y: f32, z: f32) -> Self {
        let mut m = ident();
        m[0][3] = x;
        m[1][3] = y;
        m[2][3] = z;
        self.then(m)
    }
    fn scale(self, x: f32, y: f32, z: f32) -> Self {
        let mut m = ident();
        m[0][0] = x;
        m[1][1] = y;
        m[2][2] = z;
        self.then(m)
    }
    fn rotate_x(self, a: f32) -> Self {
        let (s, c) = a.sin_cos();
        self.then([[1.0, 0.0, 0.0, 0.0], [0.0, c, -s, 0.0], [0.0, s, c, 0.0], [0.0, 0.0, 0.0, 1.0]])
    }
    fn rotate_y(self, a: f32) -> Self {
        let (s, c) = a.sin_cos();
        self.then([[c, 0.0, s, 0.0], [0.0, 1.0, 0.0, 0.0], [-s, 0.0, c, 0.0], [0.0, 0.0, 0.0, 1.0]])
    }
    fn rotate_z(self, a: f32) -> Self {
        let (s, c) = a.sin_cos();
        self.then([[c, -s, 0.0, 0.0], [s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]])
    }
    /// The display's `transformation`, row by row.
    fn tag(&self) -> Tag {
        Tag::List(self.0.iter().flatten().map(|v| Tag::Float(v.to_bits())).collect())
    }
}

/// A head icon of the menus (`IconItem.of(texture)`).
fn head_nbt(texture: &str, signature: Option<&str>) -> Compound {
    let mut prop = Compound::new();
    prop.insert(jkey("name"), sstr("textures"));
    prop.insert(jkey("value"), sstr(texture));
    if let Some(s) = signature {
        prop.insert(jkey("signature"), sstr(s));
    }
    let mut profile = Compound::new();
    profile.insert(jkey("properties"), Tag::List(vec![Tag::Compound(prop)]));
    let mut comps = Compound::new();
    comps.insert(jkey("minecraft:profile"), Tag::Compound(profile));
    let mut c = Compound::new();
    c.insert(jkey("id"), sstr("minecraft:player_head"));
    c.insert(jkey("count"), Tag::Int(1));
    c.insert(jkey("components"), Tag::Compound(comps));
    c
}

/// The owner's head, with their skin (`getProfileComponent`).
fn owner_head(g: &Grave) -> Compound {
    let mut profile = Compound::new();
    if let Some(o) = &g.owner {
        profile.insert(jkey("name"), sstr(&o.name));
        profile.insert(jkey("id"), Tag::IntArray(uuid_ints(o.uuid)));
        if let Some((value, signature)) = &o.textures {
            let mut prop = Compound::new();
            prop.insert(jkey("name"), sstr("textures"));
            prop.insert(jkey("value"), sstr(value));
            if let Some(s) = signature {
                prop.insert(jkey("signature"), sstr(s));
            }
            profile.insert(jkey("properties"), Tag::List(vec![Tag::Compound(prop)]));
        }
    }
    let mut comps = Compound::new();
    comps.insert(jkey("minecraft:profile"), Tag::Compound(profile));
    let mut c = Compound::new();
    c.insert(jkey("id"), sstr("minecraft:player_head"));
    c.insert(jkey("count"), Tag::Int(1));
    c.insert(jkey("components"), Tag::Compound(comps));
    c
}

fn texture(name: &str) -> &'static str {
    TEXTURES.iter().find(|(n, _)| *n == name).map_or("", |(_, t)| t)
}

/// The model's entities for a grave: kind, NBT, and a key of what they show.
fn model_parts(g: &Grave, state: &State) -> Vec<(&'static str, Compound)> {
    let lang = g.owner.as_ref().map_or("en_us", |_| "en_us");
    let protected = g.is_protected(state);
    let payment = g.require_payment;
    let deg = std::f32::consts::PI / 180.0;
    let mut out = Vec::new();
    let item_display = |m: Mat, item: Compound| -> Compound {
        let mut c = Compound::new();
        c.insert(jkey("item"), Tag::Compound(item));
        c.insert(jkey("item_display"), sstr("fixed"));
        c.insert(jkey("transformation"), m.tag());
        c
    };
    if !g.visual {
        // The two first items of the grave, lying by it (`ModelTags.ITEM`).
        let tools = [
            Mat::new().translate(0.4, -0.495, -0.2).rotate_y(330.0 * deg).rotate_z(5.0 * deg).rotate_x(std::f32::consts::FRAC_PI_2).scale(0.5, 0.5, 0.5),
            Mat::new().translate(-0.35, -0.43, -0.05).rotate_y(80.0 * deg).rotate_z(-30.0 * deg).rotate_x(-160.0 * deg).scale(0.5, 0.5, 0.5),
        ];
        for (k, m) in tools.into_iter().enumerate() {
            if let Some(s) = g.items.get(k).filter(|s| s.stack.count > 0) {
                if let Tag::Compound(item) = horizon_items::item_nbt::item_to_nbt(&s.stack) {
                    out.push(("minecraft:item_display", item_display(m, item)));
                }
            }
        }
    }
    // The head: the owner's while protected, a skeleton skull after.
    let head = Mat::new().translate(0.0, -0.35, 0.0).rotate_x(-std::f32::consts::PI / 12.0).rotate_z(-std::f32::consts::PI / 64.0);
    let item = if protected {
        owner_head(g)
    } else {
        let mut c = Compound::new();
        c.insert(jkey("id"), sstr("minecraft:skeleton_skull"));
        c.insert(jkey("count"), Tag::Int(1));
        c
    };
    out.push(("minecraft:item_display", item_display(head, item)));
    if payment && !g.visual {
        let lock = Mat::new().translate(0.0, -0.44, 0.42).rotate_x(-std::f32::consts::PI / 3.0).scale(0.35, 0.35, 0.35);
        out.push(("minecraft:item_display", item_display(lock, head_nbt(texture("remove_protection"), None))));
        let cost = Wrapped::of("<yellow>${cost}").with(lang, &|k| (k == "cost").then(|| cfg().unlock_cost.text(lang)));
        let m = Mat::new().translate(0.0, -0.31, 0.42).rotate_x(-std::f32::consts::PI / 3.0).scale(0.35, 0.35, 0.35);
        let mut c = Compound::new();
        c.insert(jkey("text"), nbt_of(&cost));
        c.insert(jkey("shadow"), Tag::Byte(1));
        c.insert(jkey("transformation"), m.tag());
        out.push(("minecraft:text_display", c));
    }
    // The text above (`addGenericText`): the main text of a grave, the short one of a gravestone.
    let ph = map_lookup(placeholders(g, state, lang));
    let mut lines: Vec<Value> = Vec::new();
    lines.push(Wrapped::of("<gold><lang:'text.graves.grave_of':'<yellow>${player}'>").with(lang, &ph));
    lines.push(Wrapped::of("<yellow>${death_cause}").with(lang, &ph));
    if !g.visual {
        lines.push(Wrapped::of("<gray><lang:'text.graves.items_xp':'<white>${item_count}':'<white>${xp}'>").with(lang, &ph));
        if protected && state.protection_time > 0 {
            lines.push(Wrapped::of("<blue><lang:'text.graves.protected_time':'<white>${protection_time}'><r>").with(lang, &ph));
        }
        if state.breaking_time > 0 {
            lines.push(Wrapped::of("<red><lang:'text.graves.break_time':'<white>${break_time}'>").with(lang, &ph));
        }
    }
    let mut extra = Vec::new();
    for (k, l) in lines.into_iter().enumerate() {
        if k > 0 {
            extra.push(text::literal("\n"));
        }
        extra.push(l);
    }
    let text_value = json!({ "text": "", "extra": extra });
    let m = Mat::new().translate(0.0, 0.1, 0.0).scale(0.6, 0.6, 0.6);
    let mut c = Compound::new();
    c.insert(jkey("text"), nbt_of(&text_value));
    c.insert(jkey("line_width"), Tag::Int(9999));
    c.insert(jkey("shadow"), Tag::Byte(1));
    c.insert(jkey("billboard"), sstr("center"));
    c.insert(jkey("view_range"), Tag::Float(0.5f32.to_bits()));
    let mut b = Compound::new();
    b.insert(jkey("sky"), Tag::Int(15));
    b.insert(jkey("block"), Tag::Int(15));
    c.insert(jkey("brightness"), Tag::Compound(b));
    c.insert(jkey("transformation"), m.tag());
    out.push(("minecraft:text_display", c));
    out
}

/// The tag a grave's model entities carry, besides `MODEL_TAG`.
fn grave_tag(id: i64) -> String {
    format!("universal_graves.grave_{id}")
}

/// Removes every entity of a grave's model the level holds, those saved
/// with the world too (Horizon keeps no UUID for displays to find them by).
fn kill_tagged(game: &mut Game<'_>, id: i64) {
    let _ = game.native_run_command_captured(&format!("kill @e[tag={}]", grave_tag(id)));
}

/// The model shown as it should be: parts whose look changed are summoned again.
fn refresh_model(game: &mut Game<'_>, id: i64) {
    let Some((g, parts)) = with(|s| grave_index(s, id).map(|i| (s.graves[i].clone(), model_parts(&s.graves[i], s)))) else {
        return;
    };
    if !g.cleaned {
        kill_tagged(game, id);
        with(|s| {
            if let Some(i) = grave_index(s, id) {
                s.graves[i].cleaned = true;
                s.graves[i].shown.clear();
            }
        });
        return refresh_model(game, id);
    }
    let dimension = g.dimension.clone();
    let center = [f64::from(g.pos[0]) + 0.5, f64::from(g.pos[1]) + 0.5, f64::from(g.pos[2]) + 0.5];
    let mut shown = Vec::new();
    for (k, (kind, mut nbt)) in parts.into_iter().enumerate() {
        let key = format!("{kind}{}", horizon_world::snbt_text::to_snbt(&Tag::Compound(nbt.clone())));
        if let Some((eid, old)) = g.shown.get(k) {
            if *old == key && game.native_entity(*eid).is_some() {
                shown.push((*eid, key));
                continue;
            }
            game.native_remove_entity(&dimension, *eid);
        }
        nbt.insert(jkey("Rotation"), Tag::List(vec![Tag::Float(g.yaw.to_bits()), Tag::Float(0.0f32.to_bits())]));
        nbt.insert(jkey("Tags"), Tag::List(vec![sstr(MODEL_TAG), sstr(&grave_tag(id))]));
        match game.native_summon(&dimension, kind, center, nbt) {
            Some(eid) => shown.push((eid, key)),
            None => break,
        }
    }
    // Parts no longer in the model.
    for (eid, _) in g.shown.iter().skip(shown.len()) {
        game.native_remove_entity(&dimension, *eid);
    }
    with(|s| {
        if let Some(i) = grave_index(s, id) {
            s.graves[i].shown = shown;
        }
    });
}

fn remove_model(game: &mut Game<'_>, g: &Grave) {
    for (eid, _) in &g.shown {
        game.native_remove_entity(&g.dimension, *eid);
    }
    if g.dimension == game.dim.id() && chunk_loaded(game, &g.dimension, g.pos) {
        kill_tagged(game, g.id);
    } else {
        with(|s| {
            s.orphans.push((g.dimension.clone(), g.pos, g.id));
            s.dirty = true;
        });
    }
    with(|s| {
        if let Some(x) = s.graves.iter_mut().find(|x| x.id == g.id) {
            x.shown.clear();
        }
    });
}

// ---- hooks: the world ---------------------------------------------------------------------

/// A right click on a block (the player's state swapped in): a grave opens at the tick.
pub(super) fn right_click_block(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if *canceled {
        return;
    }
    let dimension = game.dim.id().to_owned();
    if !with(|s| grave_at(s, &dimension, pos).is_some()) {
        return;
    }
    let uuid = game.native_acting_id();
    let sneaking = game.native_sneaking();
    // A stack in hand with sneaking places it (`PASS`).
    if sneaking && game.native_held().is_some() {
        return;
    }
    *canceled = true;
    with(|s| s.actions.push(Action::Use { uuid, dimension, pos, sneaking }));
}

/// A player breaking a block (`playerWillDestroy`): a grave is never broken
/// by the game; one the player may take from is broken by the mod at the tick.
pub(super) fn destroy_block(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    let dimension = game.dim.id().to_owned();
    if !with(|s| grave_at(s, &dimension, pos).is_some()) {
        return;
    }
    *canceled = true;
    let uuid = game.native_acting_id();
    with(|s| s.actions.push(Action::Break { uuid, dimension, pos }));
}

/// A player starting to dig a block: a grave, which the barrier makes
/// unbreakable in survival, breaks for one who may take from it.
pub(super) fn left_click_block(game: &mut Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    destroy_block(game, pos, canceled);
}

/// An explosion's blocks: graves stay (`strength(2, 999)`).
pub(super) fn explosion(dimension: &str, blocks: &mut Vec<[i32; 3]>) {
    with(|s| blocks.retain(|p| grave_at(s, dimension, *p).is_none()));
}

/// An item used in the air: the grave compass opens its grave (`GraveCompassItem.use`).
pub(super) fn use_item(game: &mut Game<'_>, stack: &ItemStack) {
    let Some(grave) = compass_grave(stack) else {
        return;
    };
    if !cfg().compass_opens_gui {
        return;
    }
    let uuid = game.native_acting_id();
    with(|s| s.actions.push(Action::Compass { uuid, grave }));
}

fn compass_grave(stack: &ItemStack) -> Option<i64> {
    if &*stack.item != "minecraft:compass" {
        return None;
    }
    let Tag::Compound(c) = horizon_items::item_nbt::item_to_nbt(stack) else {
        return None;
    };
    let Some(Tag::Compound(comps)) = c.get(&jkey("components")) else {
        return None;
    };
    let Some(Tag::Compound(data)) = comps.get(&jkey("minecraft:custom_data")) else {
        return None;
    };
    let Some(Tag::Compound(mark)) = data.get(&jkey(COMPASS_KEY)) else {
        return None;
    };
    get_i64(mark, "grave")
}

/// `GraveCompassItem.create`: a compass to the grave, its name the mod's.
fn compass(game: &Game<'_>, grave: &Grave, lang: &str, to_vanilla: bool) -> Option<ItemStack> {
    let mut mark = Compound::new();
    mark.insert(jkey("grave"), Tag::Long(grave.id));
    mark.insert(jkey("to_vanilla"), Tag::Byte(i8::from(to_vanilla)));
    let mut data = Compound::new();
    data.insert(jkey(COMPASS_KEY), Tag::Compound(mark));
    let mut target = Compound::new();
    target.insert(jkey("dimension"), sstr(&grave.dimension));
    target.insert(jkey("pos"), Tag::IntArray(grave.pos.to_vec()));
    let mut tracker = Compound::new();
    tracker.insert(jkey("target"), Tag::Compound(target));
    tracker.insert(jkey("tracked"), Tag::Byte(0));
    let mut comps = Compound::new();
    comps.insert(jkey("minecraft:custom_data"), Tag::Compound(data));
    comps.insert(jkey("minecraft:lodestone_tracker"), Tag::Compound(tracker));
    comps.insert(jkey("minecraft:max_stack_size"), Tag::Int(1));
    comps.insert(jkey("minecraft:enchantment_glint_override"), Tag::Byte(1));
    comps.insert(
        jkey("minecraft:item_name"),
        nbt_of(&text::translatable(lang, "item.universal_graves.grave_compass", Vec::new())),
    );
    let mut c = Compound::new();
    c.insert(jkey("id"), sstr("minecraft:compass"));
    c.insert(jkey("count"), Tag::Int(1));
    c.insert(jkey("components"), Tag::Compound(comps));
    let _ = game;
    horizon_items::item_nbt::item_from_nbt(&Tag::Compound(c))
}

/// A player respawned: their grave compass at the tick (`initInventoryMenu`).
pub(super) fn respawned(player: u128) {
    with(|s| s.respawned.push(player));
}

// ---- hooks: menus and commands ------------------------------------------------------------

pub(super) fn menu_clicked(uuid: u128, window: i32, slot: i32) {
    with(|s| {
        if s.guis.get(&uuid).is_some_and(|g| g.window == window) {
            s.actions.push(Action::Click { uuid, window, slot });
        }
    });
}

pub(super) fn menu_closed(uuid: u128, window: i32) {
    with(|s| s.actions.push(Action::Closed { uuid, window }));
}

pub(super) fn register_commands(commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    use horizon_client::mod_api::NativeCommand as C;
    commands.push(C::new("graves"));
    commands.push(C::new("graves about"));
    commands.push(C::op("graves modify", 3));
    commands.push(C::op("graves player <player:word>", 3));
    commands.push(C::op("graves player <player:word> modify", 3));
    commands.push(C::op("graves all", 3));
    commands.push(C::op("graves all modify", 3));
    commands.push(C::op("graves reload", 4));
}

pub(super) fn perform_command(player: &horizon_client::mod_api::NativeServerPlayer, command: &str, ran: &mut bool) {
    let args: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
    if args.first().map(String::as_str) != Some("graves") || *ran {
        return;
    }
    *ran = true;
    let (uuid, permission) = (player.uuid, player.permission);
    with(|s| s.actions.push(Action::Command { uuid, permission, args }));
}

// ---- the menus ----------------------------------------------------------------------------

/// A menu icon: the config's item with its name and lore lines.
fn icon_stack(icon: &Icon, lang: &str, ph: &dyn Fn(&str) -> Option<Value>) -> Option<ItemStack> {
    let mut c = match &icon.stack {
        Value::String(id) => {
            let mut c = Compound::new();
            c.insert(jkey("id"), sstr(id));
            c.insert(jkey("count"), Tag::Int(1));
            c
        }
        Value::Object(o) => {
            let id = o.get("id").and_then(Value::as_str).unwrap_or("minecraft:stone");
            let tex = o.get("components").and_then(|c| c.get("universal_graves:texture")).and_then(Value::as_str);
            match (id, tex) {
                ("universal_graves:icon", Some(t)) => head_nbt(texture(t), None),
                _ => {
                    let mut c = Compound::new();
                    c.insert(jkey("id"), sstr(id));
                    c.insert(jkey("count"), Tag::Int(o.get("count").and_then(Value::as_i64).unwrap_or(1) as i32));
                    if let Some(comps) = o.get("components") {
                        c.insert(jkey("components"), nbt_of(comps));
                    }
                    c
                }
            }
        }
        _ => return None,
    };
    let mut comps = match c.get(&jkey("components")) {
        Some(Tag::Compound(x)) => x.clone(),
        _ => Compound::new(),
    };
    let mut lines = icon.text.iter().map(|w| text::not_italic(w.with(lang, ph)));
    if let Some(name) = lines.next() {
        comps.insert(jkey("minecraft:custom_name"), nbt_of(&name));
    }
    let lore: Vec<Tag> = lines.map(|l| nbt_of(&l)).collect();
    if !lore.is_empty() {
        comps.insert(jkey("minecraft:lore"), Tag::List(lore));
    }
    // `hideDefaultTooltip`.
    let mut hide = Compound::new();
    hide.insert(
        jkey("hidden_components"),
        Tag::List(
            ["minecraft:attribute_modifiers", "minecraft:enchantments", "minecraft:profile", "minecraft:lodestone_tracker"]
                .iter()
                .map(|s| sstr(s))
                .collect(),
        ),
    );
    comps.insert(jkey("minecraft:tooltip_display"), Tag::Compound(hide));
    c.insert(jkey("components"), Tag::Compound(comps));
    horizon_items::item_nbt::item_from_nbt(&Tag::Compound(c))
}

/// Who may do what with a grave in a menu (`GraveGui`'s fields).
struct Rights {
    has_access: bool,
    can_take: bool,
    creative: bool,
}

fn rights(game: &Game<'_>, g: &Grave, uuid: u128, state: &State) -> Rights {
    let creative = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|x| x.uuid == uuid)).is_some_and(|x| x.game_mode == 1);
    let permission = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|x| x.uuid == uuid)).map_or(0, |x| x.permission);
    // `canTakeFrom`/`hasAccess`, and a creative operator's bypass (`universal_graves.bypass`).
    let bypass = creative && permission >= 3;
    let has_access = g.has_access(uuid, state) || bypass;
    let can_take = (!g.require_payment && g.has_access(uuid, state)) || bypass;
    Rights { has_access, can_take, creative }
}

/// The menu's slots and what a click on each does (`PagedGui.updateDisplay`).
fn render(game: &Game<'_>, uuid: u128, gui: &mut Gui) -> (Value, Vec<Option<ItemStack>>) {
    let c = cfg();
    let lang = language(game, uuid);
    let mut slots: Vec<Option<ItemStack>> = vec![None; PAGE + 9];
    let mut clicks: Vec<Option<Click>> = vec![None; PAGE + 9];
    let bar = icon_stack(&c.bar, &lang, &|_| None);
    let title;
    match gui.kind.clone() {
        Kind::List { .. } | Kind::All => {
            let all = matches!(gui.kind, Kind::All);
            let graves: Vec<(i64, bool, HashMap<&'static str, Value>)> = with(|s| {
                s.graves
                    .iter()
                    .filter(|g| !g.removed && !g.visual && !g.should_break(s))
                    .filter(|g| match &gui.kind {
                        Kind::List { target, .. } => g.is_owner(*target),
                        _ => true,
                    })
                    .map(|g| (g.id, g.is_protected(s), placeholders(g, s, &lang)))
                    .collect()
            });
            title = match &gui.kind {
                Kind::List { name, .. } => {
                    let name = name.clone();
                    c.title.with(&lang, &move |k| (k == "player").then(|| text::literal(&name)))
                }
                _ => c.admin_title.text(&lang),
            };
            let pages = graves.len() / PAGE + 1;
            gui.page = gui.page.min(pages - 1);
            for (k, (id, protected, ph)) in graves.into_iter().skip(gui.page * PAGE).take(PAGE).enumerate() {
                let icon = if all { c.admin_list_icon.get(protected) } else { c.list_icon.get(protected) };
                slots[k] = icon_stack(icon, &lang, &map_lookup(ph));
                clicks[k] = Some(Click::Open(id));
            }
            for n in 0..9 {
                slots[PAGE + n] = bar.clone();
            }
            nav_page(&c, &lang, gui.page, pages, 2, 6, &mut slots, &mut clicks);
        }
        Kind::Grave { id, previous } => {
            let Some((g, protected, ph)) = with(|s| {
                grave_index(s, id).map(|i| {
                    let g = s.graves[i].clone();
                    let p = g.is_protected(s);
                    let ph = placeholders(&g, s, &lang);
                    (g, p, ph)
                })
            }) else {
                return (text::literal(""), slots);
            };
            let rights = with(|s| rights(game, &g, uuid, s));
            title = c.title.with(&lang, &map_lookup(ph.clone()));
            let pages = g.items.len() / PAGE + 1;
            gui.page = gui.page.min(pages - 1);
            for k in 0..PAGE {
                let i = gui.page * PAGE + k;
                if let Some(s) = g.items.get(i).filter(|s| s.stack.count > 0) {
                    slots[k] = Some(s.stack.clone());
                    if gui.can_modify && rights.can_take {
                        clicks[k] = Some(Click::Item(i));
                    }
                }
            }
            for n in 0..9 {
                slots[PAGE + n] = bar.clone();
            }
            // 0: the grave's info.
            slots[PAGE] = icon_stack(c.info_icon.get(protected), &lang, &map_lookup(ph.clone()));
            // 1: unlocking.
            let remote_unlock = c.remote_unlocking || gui.can_modify;
            if g.require_payment && remote_unlock {
                let ok = cost_ok(game, uuid, &c.unlock_cost);
                let cost = c.unlock_cost.text(&lang);
                slots[PAGE + 1] = icon_stack(c.unlock.get(ok), &lang, &move |k| (k == "cost").then(|| cost.clone()));
                clicks[PAGE + 1] = Some(Click::Unlock);
            }
            // 2: the protection's removal, or breaking the grave.
            let confirming = gui.confirm.is_some();
            if protected && rights.has_access && (c.remote_protection_removal || gui.can_modify) {
                slots[PAGE + 2] = icon_stack(c.remove_protection.get(!confirming), &lang, &|_| None);
                clicks[PAGE + 2] = Some(Click::RemoveProtection);
            } else if gui.can_modify || (rights.can_take && c.remote_breaking) {
                slots[PAGE + 2] = icon_stack(c.break_grave.get(!confirming), &lang, &|_| None);
                clicks[PAGE + 2] = Some(Click::BreakGrave);
            }
            // 3: taking everything, or teleporting.
            let can_teleport = c.tp_cost.kind != "creative" || rights.creative;
            if rights.can_take && gui.can_modify {
                slots[PAGE + 3] = icon_stack(&c.quick_pickup, &lang, &|_| None);
                clicks[PAGE + 3] = Some(Click::QuickPickup);
            } else if can_teleport {
                let ok = cost_ok(game, uuid, &c.tp_cost);
                let cost = c.tp_cost.text(&lang);
                slots[PAGE + 3] = icon_stack(c.teleport.get(ok), &lang, &move |k| (k == "cost").then(|| cost.clone()));
                clicks[PAGE + 3] = Some(Click::Teleport);
            }
            // 4: fetching the grave.
            if gui.can_fetch {
                slots[PAGE + 4] = icon_stack(c.fetch.get(gui.confirm_fetch.is_none()), &lang, &|_| None);
                clicks[PAGE + 4] = Some(Click::Fetch);
            }
            // 5..8: the pages and the way back.
            let has_previous = previous.is_some();
            if has_previous {
                nav_page(&c, &lang, gui.page, pages, 5, 6, &mut slots, &mut clicks);
                slots[PAGE + 8] = icon_stack(&c.back, &lang, &|_| None);
                clicks[PAGE + 8] = Some(Click::Back);
            } else {
                nav_page(&c, &lang, gui.page, pages, 5, 7, &mut slots, &mut clicks);
            }
        }
    }
    gui.clicks = clicks;
    (title, slots)
}

#[allow(clippy::too_many_arguments)]
fn nav_page(c: &Config, lang: &str, page: usize, pages: usize, prev: usize, next: usize, slots: &mut [Option<ItemStack>], clicks: &mut [Option<Click>]) {
    slots[PAGE + prev] = icon_stack(c.previous.get(page > 0), lang, &|_| None);
    clicks[PAGE + prev] = Some(Click::Previous);
    slots[PAGE + next] = icon_stack(c.next.get(page + 1 < pages), lang, &|_| None);
    clicks[PAGE + next] = Some(Click::Next);
}

fn open_gui(game: &mut Game<'_>, uuid: u128, kind: Kind, can_modify: bool, can_fetch: bool) {
    // A dead player keeps the death screen.
    if guest_index(game, uuid).is_none_or(|gi| game.lan.as_ref().unwrap().hub.guests[gi].dead) {
        return;
    }
    let mut gui = Gui { kind, window: -1, page: 0, can_modify, can_fetch, ticker: 0, confirm: None, confirm_fetch: None, clicks: Vec::new() };
    let (title, slots) = render(game, uuid, &mut gui);
    if let Some(window) = game.native_open_menu(uuid, ROWS, to_text(&title), slots) {
        gui.window = window;
        with(|s| s.guis.insert(uuid, gui));
    }
}

fn update_gui(game: &mut Game<'_>, uuid: u128) {
    let Some(mut gui) = with(|s| s.guis.remove(&uuid)) else {
        return;
    };
    if let Kind::Grave { id, previous } = &gui.kind
        && with(|s| grave_index(s, *id).is_none())
    {
        // `onTick`: a removed grave sends the player back, or closes.
        match previous.clone() {
            Some(p) => open_gui(game, uuid, *p, gui.can_modify, gui.can_fetch),
            None => game.native_close_menu(uuid),
        }
        return;
    }
    let (_, slots) = render(game, uuid, &mut gui);
    game.native_set_menu_items(uuid, slots);
    with(|s| s.guis.insert(uuid, gui));
}

fn click(game: &mut Game<'_>, uuid: u128, window: i32, slot: i32) {
    let Some(gui) = with(|s| s.guis.get(&uuid).filter(|g| g.window == window).map(|g| (g.clicks.clone(), g.kind.clone(), g.can_modify, g.can_fetch)))
    else {
        return;
    };
    let (clicks, kind, can_modify, can_fetch) = gui;
    let Some(Some(action)) = usize::try_from(slot).ok().and_then(|k| clicks.get(k).copied()) else {
        return;
    };
    let c = cfg();
    let set = |f: &dyn Fn(&mut Gui)| with(|s| s.guis.get_mut(&uuid).map(f));
    match action {
        Click::Previous => {
            click_sound(game, uuid);
            set(&|g| g.page = g.page.saturating_sub(1));
        }
        Click::Next => {
            click_sound(game, uuid);
            set(&|g| g.page += 1);
        }
        Click::Back => {
            click_sound(game, uuid);
            if let Kind::Grave { previous: Some(p), .. } = kind {
                open_gui(game, uuid, *p, can_modify, can_fetch);
                return;
            }
        }
        Click::Open(id) => {
            open_gui(game, uuid, Kind::Grave { id, previous: Some(Box::new(kind)) }, can_modify, can_fetch);
            return;
        }
        Click::Item(i) => {
            if let Kind::Grave { id, .. } = kind {
                take_one(game, uuid, id, i);
            }
        }
        Click::QuickPickup => {
            if let Kind::Grave { id, .. } = kind {
                click_sound(game, uuid);
                quick_equip(game, uuid, id);
            }
        }
        Click::Unlock => {
            if let Kind::Grave { id, .. } = kind {
                if pay_for_unlock(game, uuid, id) {
                    click_sound(game, uuid);
                } else {
                    sound(game, uuid, "minecraft:entity.villager.no", 0.5, 1.0);
                }
            }
        }
        Click::RemoveProtection => {
            if let Kind::Grave { id, .. } = kind {
                click_sound(game, uuid);
                if with(|s| s.guis.get(&uuid).is_some_and(|g| g.confirm.is_some())) {
                    with(|s| {
                        if let Some(i) = grave_index(s, id) {
                            s.graves[i].protection_enabled = false;
                            s.dirty = true;
                        }
                    });
                    set(&|g| g.confirm = None);
                } else {
                    set(&|g| g.confirm = Some(g.ticker + 100));
                }
            }
        }
        Click::BreakGrave => {
            if let Kind::Grave { id, .. } = kind {
                click_sound(game, uuid);
                if with(|s| s.guis.get(&uuid).is_some_and(|g| g.confirm.is_some())) {
                    destroy_grave(game, id, Some(uuid));
                    with(|s| s.guis.remove(&uuid));
                    game.native_close_menu(uuid);
                    return;
                }
                set(&|g| g.confirm = Some(g.ticker + 100));
            }
        }
        Click::Teleport => {
            if let Kind::Grave { id, .. } = kind {
                if take_cost(game, uuid, &c.tp_cost) {
                    click_sound(game, uuid);
                    with(|s| s.guis.remove(&uuid));
                    game.native_close_menu(uuid);
                    teleport_to_grave(game, uuid, id);
                    return;
                }
                sound(game, uuid, "minecraft:entity.villager.no", 0.5, 1.0);
            }
        }
        Click::Fetch => {
            if let Kind::Grave { id, .. } = kind {
                click_sound(game, uuid);
                if with(|s| s.guis.get(&uuid).is_some_and(|g| g.confirm_fetch.is_some())) {
                    set(&|g| g.confirm_fetch = None);
                    if fetch_grave(game, uuid, id) {
                        with(|s| s.guis.remove(&uuid));
                        game.native_close_menu(uuid);
                        return;
                    }
                } else {
                    set(&|g| g.confirm_fetch = Some(g.ticker + 100));
                }
            }
        }
    }
    update_gui(game, uuid);
}

// ---- experience (`Player.giveExperiencePoints`, `giveExperienceLevels`) ---------------------

/// `getXpNeededForNextLevel`.
fn xp_needed(level: i32) -> f32 {
    (if level >= 30 {
        112 + (level - 30) * 9
    } else if level >= 15 {
        37 + (level - 15) * 5
    } else {
        7 + level * 2
    }) as f32
}

/// `Player.giveExperiencePoints` on a player of the hub (their bar follows).
fn give_points(game: &mut Game<'_>, uuid: u128, points: i32) {
    let Some(gi) = guest_index(game, uuid) else {
        return;
    };
    let g = &mut game.lan.as_mut().unwrap().hub.guests[gi];
    g.xp_progress += points as f32 / xp_needed(g.xp_level);
    g.xp_total = g.xp_total.saturating_add(points).max(0);
    while g.xp_progress >= 1.0 {
        g.xp_progress = (g.xp_progress - 1.0) * xp_needed(g.xp_level);
        g.xp_level += 1;
        g.xp_progress /= xp_needed(g.xp_level);
    }
}

/// `Player.giveExperienceLevels`.
fn give_levels(game: &mut Game<'_>, uuid: u128, levels: i32) {
    let Some(gi) = guest_index(game, uuid) else {
        return;
    };
    let g = &mut game.lan.as_mut().unwrap().hub.guests[gi];
    g.xp_level += levels;
    if g.xp_level < 0 {
        g.xp_level = 0;
        g.xp_progress = 0.0;
        g.xp_total = 0;
    }
}

// ---- costs (`GenericCost`) ----------------------------------------------------------------

fn guest_info(game: &Game<'_>, uuid: u128) -> Option<(i32, i32)> {
    game.lan.as_ref()?.hub.guests.iter().find(|g| g.uuid == uuid).map(|g| (g.game_mode, g.xp_level))
}

fn cost_ok(game: &Game<'_>, uuid: u128, cost: &Cost) -> bool {
    check_cost(game, uuid, cost)
}

fn check_cost(game: &Game<'_>, uuid: u128, cost: &Cost) -> bool {
    let Some((mode, level)) = guest_info(game, uuid) else {
        return false;
    };
    if mode == 1 {
        return true;
    }
    match cost.kind.as_str() {
        "free" => true,
        "creative" => false,
        "level" => level >= cost.count,
        "item" => {
            let id = cost_item(cost);
            game.lan
                .as_ref()
                .and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid))
                .map_or(0, |g| g.inventory.iter().flatten().filter(|s| *s.item == *id).map(|s| s.count as i32).sum::<i32>())
                >= cost.count
        }
        _ => false,
    }
}

fn cost_item(cost: &Cost) -> String {
    match &cost.input {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(o)) => o.get("id").and_then(Value::as_str).unwrap_or("").to_owned(),
        _ => String::new(),
    }
}

fn take_cost(game: &mut Game<'_>, uuid: u128, cost: &Cost) -> bool {
    if !check_cost(game, uuid, cost) {
        return false;
    }
    let Some(gi) = guest_index(game, uuid) else {
        return false;
    };
    let host = game.lan.as_mut().unwrap();
    let g = &mut host.hub.guests[gi];
    if g.game_mode == 1 {
        return true;
    }
    match cost.kind.as_str() {
        "level" => {
            give_levels(game, uuid, -cost.count);
        }
        "item" => {
            let id = cost_item(cost);
            let mut left = cost.count as u32;
            for place in g.inventory.iter_mut() {
                if left == 0 {
                    break;
                }
                if let Some(s) = place.as_mut().filter(|s| *s.item == *id) {
                    let n = s.count.min(left);
                    s.count -= n;
                    left -= n;
                    if s.count == 0 {
                        *place = None;
                    }
                }
            }
            host.hub.send_inventory(gi);
        }
        _ => {}
    }
    true
}

fn return_cost(game: &mut Game<'_>, uuid: u128, cost: &Cost) {
    let Some(gi) = guest_index(game, uuid) else {
        return;
    };
    if game.lan.as_ref().unwrap().hub.guests[gi].game_mode == 1 {
        return;
    }
    match cost.kind.as_str() {
        "level" => {
            give_levels(game, uuid, cost.count);
        }
        "item" => {
            let id = cost_item(cost);
            let mut c = Compound::new();
            c.insert(jkey("id"), sstr(&id));
            c.insert(jkey("count"), Tag::Int(cost.count));
            if let Some(stack) = horizon_items::item_nbt::item_from_nbt(&Tag::Compound(c)) {
                give(game, uuid, stack);
            }
        }
        _ => {}
    }
}

/// `Grave.payForUnlock`.
fn pay_for_unlock(game: &mut Game<'_>, uuid: u128, id: i64) -> bool {
    let c = cfg();
    let lang = language(game, uuid);
    let access = with(|s| grave_index(s, id).is_some_and(|i| s.graves[i].has_access(uuid, s)));
    if !access {
        if !c.no_access.is_empty() {
            tell(game, uuid, c.no_access.text(&lang));
        }
        return false;
    }
    let cost = c.unlock_cost.text(&lang);
    let ph = move |k: &str| match k {
        "cost" => Some(cost.clone()),
        _ => None,
    };
    if take_cost(game, uuid, &c.unlock_cost) {
        with(|s| {
            if let Some(i) = grave_index(s, id) {
                s.graves[i].require_payment = false;
                s.dirty = true;
            }
        });
        if !c.accepted.is_empty() {
            tell(game, uuid, c.accepted.with(&lang, &ph));
        }
        return true;
    }
    tell(game, uuid, c.payment_failed.with(&lang, &ph));
    false
}

// ---- taking items back (`quickEquip`, the masks' `moveToPlayer*`) --------------------------

/// Puts a stored stack where it was (`moveToPlayerExactly`): true when it went.
fn move_exactly(game: &mut Game<'_>, uuid: u128, s: &Stored) -> bool {
    match &s.mask {
        Mask::Vanilla => {
            let Some(gi) = guest_index(game, uuid) else {
                return false;
            };
            let g = &mut game.lan.as_mut().unwrap().hub.guests[gi];
            let place = match s.slot {
                0..=35 => g.inventory.get_mut(s.slot as usize),
                36..=39 => g.armor.get_mut(39 - s.slot as usize),
                40 => Some(&mut g.offhand),
                _ => None,
            };
            match place {
                Some(p) if p.is_none() => {
                    *p = Some(s.stack.clone());
                    true
                }
                _ => false,
            }
        }
        Mask::Accessories(slot, cosmetic) => accessory_put(game, uuid, slot, *cosmetic, Some(s.slot), &s.stack),
    }
}

/// An Accessories slot, `index` or the first free one (`moveToPlayerClosest`).
fn accessory_put(game: &mut Game<'_>, uuid: u128, slot: &str, cosmetic: bool, index: Option<i32>, stack: &ItemStack) -> bool {
    const HOLDER: &str = "accessories:inventory_holder";
    let Some(Tag::Compound(mut all)) = game.native_player_persistent_data(uuid) else {
        return false;
    };
    let Some(Tag::Compound(mut holder)) = all.get(&jkey(HOLDER)).cloned() else {
        return false;
    };
    let Some(Tag::Compound(mut containers)) = holder.get(&jkey("accessories_containers")).cloned() else {
        return false;
    };
    let Some(Tag::Compound(mut sc)) = containers.get(&jkey(slot)).cloned() else {
        return false;
    };
    let size = get_i64(&sc, "current_size").unwrap_or(1) as i32;
    let key = if cosmetic { "cosmetics" } else { "items" };
    let mut list = match sc.get(&jkey(key)) {
        Some(Tag::List(l)) => l.clone(),
        _ => Vec::new(),
    };
    let used: HashSet<i32> = list
        .iter()
        .filter_map(|t| match t {
            Tag::Compound(c) if c.get(&jkey("id")).is_some() => get_i64(c, "Slot").map(|v| v as i32),
            _ => None,
        })
        .collect();
    let free = match index {
        Some(i) => (i < size && !used.contains(&i)).then_some(i),
        None => (0..size).find(|i| !used.contains(i)),
    };
    let Some(i) = free else {
        return false;
    };
    let Tag::Compound(mut item) = horizon_items::item_nbt::item_to_nbt(stack) else {
        return false;
    };
    item.insert(jkey("Slot"), Tag::Byte(i as i8));
    list.retain(|t| !matches!(t, Tag::Compound(c) if get_i64(c, "Slot") == Some(i64::from(i))));
    list.push(Tag::Compound(item));
    sc.insert(jkey(key), Tag::List(list));
    containers.insert(jkey(slot), Tag::Compound(sc));
    holder.insert(jkey("accessories_containers"), Tag::Compound(containers));
    all.insert(jkey(HOLDER), Tag::Compound(holder));
    game.native_set_player_persistent_data(uuid, Tag::Compound(all))
}

/// `Grave.quickEquip`: every item back where it was, else where it fits;
/// the experience too; then the grave breaks if it is empty.
fn quick_equip(game: &mut Game<'_>, uuid: u128, id: i64) {
    let c = cfg();
    let Some(grave) = with(|s| grave_index(s, id).filter(|&i| s.graves[i].has_access(uuid, s)).map(|i| s.graves[i].clone())) else {
        return;
    };
    if guest_index(game, uuid).is_none_or(|gi| game.lan.as_ref().unwrap().hub.guests[gi].dead) {
        return;
    }
    if grave.require_payment && !pay_for_unlock(game, uuid, id) {
        return;
    }
    let _ = c;
    let mut rest = Vec::new();
    for s in &grave.items {
        if s.stack.count > 0 && !move_exactly(game, uuid, s) {
            rest.push(s.clone());
        }
    }
    for s in rest {
        let placed = match &s.mask {
            Mask::Accessories(slot, cosmetic) => accessory_put(game, uuid, slot, *cosmetic, None, &s.stack),
            Mask::Vanilla => false,
        };
        if !placed {
            give(game, uuid, s.stack);
        }
    }
    if let Some(gi) = guest_index(game, uuid) {
        game.lan.as_mut().unwrap().hub.send_inventory(gi);
    }
    if grave.xp > 0 {
        give_points(game, uuid, grave.xp);
    }
    with(|s| {
        if let Some(i) = grave_index(s, id) {
            s.graves[i].items.clear();
            s.graves[i].xp = 0;
            s.dirty = true;
        }
    });
    try_break(game, id, Some(uuid));
}

/// A click on one of a grave's items in its menu: it goes to the player.
fn take_one(game: &mut Game<'_>, uuid: u128, id: i64, i: usize) {
    let Some(stored) = with(|s| {
        let gi = grave_index(s, id)?;
        let st = s.graves[gi].items.get(i)?.clone();
        if st.stack.count == 0 {
            return None;
        }
        s.graves[gi].items[i].stack.count = 0;
        s.dirty = true;
        Some(st)
    }) else {
        return;
    };
    give(game, uuid, stored.stack);
}

/// `Grave.tryBreak`: an expired or emptied grave goes.
fn try_break(game: &mut Game<'_>, id: i64, player: Option<u128>) -> bool {
    let go = with(|s| grave_index(s, id).is_some_and(|i| !s.graves[i].visual && (s.graves[i].should_break(s) || s.graves[i].is_empty())));
    go && destroy_grave(game, id, player)
}

/// The grave to the player (`fetch`).
fn fetch_grave(game: &mut Game<'_>, uuid: u128, id: i64) -> bool {
    let Some(g) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)) else {
        return false;
    };
    let (dimension, pos) = (g.dimension.clone(), g.position.map(|v| v.floor() as i32));
    move_grave(game, id, &dimension, pos)
}

/// `Grave.moveTo`: the grave to `pos`, in the level in place.
fn move_grave(game: &mut Game<'_>, id: i64, dimension: &str, pos: [i32; 3]) -> bool {
    let dimension = dimension.to_owned();
    if dimension != game.dim.id() || !can_replace(game, pos, cfg().replace_any) {
        return false;
    }
    let Some(old) = with(|s| grave_index(s, id).map(|i| s.graves[i].clone())) else {
        return false;
    };
    if old.dimension != game.dim.id() {
        return false;
    }
    remove_model(game, &old);
    restore_block(game, &old);
    let Some(state) = game.native_default_state(GRAVE_BLOCK) else {
        return false;
    };
    let replaced_state = game.world.block(pos[0], pos[1], pos[2]);
    let replaced = Some((replaced_state.0, game.state_string(replaced_state)));
    if !game.native_set_block(&dimension, pos, state, 3) {
        return false;
    }
    with(|s| {
        if let Some(i) = grave_index(s, id) {
            s.graves[i].pos = pos;
            s.graves[i].dimension = dimension;
            s.graves[i].replaced = replaced;
            s.graves[i].shown.clear();
            s.dirty = true;
        }
    });
    true
}

/// `GraveUtils.teleportToGrave`: a countdown, then the jump.
fn teleport_to_grave(game: &mut Game<'_>, uuid: u128, id: i64) {
    let c = cfg();
    let Some((dimension, pos)) = with(|s| grave_index(s, id).map(|i| (s.graves[i].dimension.clone(), s.graves[i].pos))) else {
        return;
    };
    let Some((mode, _)) = guest_info(game, uuid) else {
        return;
    };
    let creative = mode == 1;
    let lang = language(game, uuid);
    let w = if c.tp_moving || creative { &c.tp_timer_moving } else { &c.tp_timer };
    let time = if creative { "0".to_owned() } else { c.tp_time.to_string() };
    tell(game, uuid, w.with(&lang, &move |k| (k == "time").then(|| text::literal(&time))));
    let from = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map_or([0.0; 3], |g| g.position);
    let to = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + c.tp_height + 1.0, f64::from(pos[2]) + 0.5];
    let ticks = if creative { 1 } else { c.tp_time * 20 };
    with(|s| s.teleports.push(Teleport { uuid, dimension, to, from, ticks, refund: true }));
}

fn tick_teleports(game: &mut Game<'_>) {
    let c = cfg();
    let list = with(|s| std::mem::take(&mut s.teleports));
    let mut keep = Vec::new();
    for mut t in list {
        t.ticks -= 1;
        let Some(g) = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == t.uuid)) else {
            continue;
        };
        let (pos, creative) = (g.position, g.game_mode == 1);
        let lang = language(game, t.uuid);
        if !c.tp_moving && !creative && pos != t.from {
            tell(game, t.uuid, c.tp_canceled.text(&lang));
            sound(game, t.uuid, "minecraft:entity.shulker.hurt_closed", 1.0, 0.5);
            if t.refund {
                return_cost(game, t.uuid, &c.tp_cost);
            }
            continue;
        }
        if t.ticks <= 0 {
            let position = text::translatable(
                &lang,
                "chat.coordinates",
                vec![text::literal(&format!("{}", t.to[0] - 0.5)), text::literal(&format!("{}", t.to[1] - 1.0)), text::literal(&format!("{}", t.to[2] - 0.5))],
            );
            tell(game, t.uuid, c.tp_location.with(&lang, &move |k| (k == "position").then(|| position.clone())));
            let _ = game.native_teleport_player(t.uuid, &t.dimension, t.to, 0.0, 0.0);
            sound(game, t.uuid, "minecraft:entity.enderman.teleport", 1.0, 1.0);
            continue;
        }
        keep.push(t);
    }
    with(|s| s.teleports.extend(keep));
}

// ---- the tick -----------------------------------------------------------------------------

/// The world opened (or another did): its graves, the config.
fn ensure_world(game: &Game<'_>) {
    let dir = game.native_world_dir();
    let same = with(|s| s.world == dir);
    if same {
        return;
    }
    load_config();
    with(|s| {
        if s.world.is_some() && s.dirty {
            save_graves(s);
        }
        let rules = std::mem::take(&mut s.accessory_rules);
        *s = State::default();
        s.accessory_rules = rules;
        s.world = dir.clone();
        if let Some(d) = &dir {
            load_graves(s, d);
        }
        let c = cfg();
        s.protection_time = c.protection_time;
        s.breaking_time = c.breaking_time;
    });
}

/// Each level's tick, with the hub in hand.
pub(super) fn level_tick(game: &mut Game<'_>) {
    ensure_world(game);
    if with(|s| s.world.is_none()) {
        return;
    }
    let dimension = game.dim.id().to_owned();
    let now = game.world.game_time;
    // Once a server tick: the second's clock (`GraveManager.tick`), the menus, teleports.
    let first = with(|s| s.last_tick.replace(now) != Some(now));
    if first {
        let second = with(|s| {
            s.ticker += 1;
            if s.ticker >= 20 {
                s.ticker = 0;
                s.game_time += 1;
                let c = cfg();
                s.protection_time = c.protection_time;
                s.breaking_time = c.breaking_time;
                true
            } else {
                false
            }
        });
        for (uuid, v) in with(|s| std::mem::take(&mut s.messages)) {
            tell(game, uuid, v);
        }
        for uuid in with(|s| std::mem::take(&mut s.respawned)) {
            give_compass(game, uuid);
        }
        let actions = with(|s| std::mem::take(&mut s.actions));
        let mut later = Vec::new();
        for a in actions {
            let elsewhere = matches!(&a, Action::Use { dimension: d, .. } | Action::Break { dimension: d, .. } if *d != dimension);
            if elsewhere {
                later.push(a);
            } else {
                act(game, a);
            }
        }
        with(|s| s.actions.extend(later));
        tick_teleports(game);
        // `onTick` of the open menus: their timers, every second.
        let open: Vec<u128> = with(|s| {
            s.guis
                .iter_mut()
                .filter_map(|(u, g)| {
                    g.ticker += 1;
                    if g.confirm.is_some_and(|t| t <= g.ticker) {
                        g.confirm = None;
                    }
                    if g.confirm_fetch.is_some_and(|t| t <= g.ticker) {
                        g.confirm_fetch = None;
                    }
                    (g.ticker % 20 == 0).then_some(*u)
                })
                .collect()
        });
        for u in open {
            update_gui(game, u);
        }
        if second {
            check_compasses(game);
            if with(|s| s.dirty && s.game_time % 30 == 0) {
                with(save_graves);
            }
        }
    } else {
        let actions = with(|s| std::mem::take(&mut s.actions));
        let (here, later): (Vec<_>, Vec<_>) = actions.into_iter().partition(|a| match a {
            Action::Use { dimension: d, .. } | Action::Break { dimension: d, .. } => *d == dimension,
            _ => false,
        });
        with(|s| s.actions.extend(later));
        for a in here {
            act(game, a);
        }
    }
    // The deaths in this level.
    let pending: Vec<Pending> = with(|s| {
        let (here, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.pending).into_iter().partition(|p| p.grave.dimension == dimension);
        s.pending = rest;
        here
    });
    for p in pending {
        place_pending(game, p);
    }
    // This level's graves, once a second (`Grave.tick` → `updateSelf`).
    let second = with(|s| s.game_time);
    if with(|s| s.dim_second.get(&dimension) == Some(&second)) {
        return;
    }
    with(|s| s.dim_second.insert(dimension.clone(), second));
    let orphans: Vec<([i32; 3], i64)> = with(|s| {
        let (here, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut s.orphans).into_iter().partition(|(d, _, _)| *d == dimension);
        s.orphans = rest;
        here.into_iter().map(|(_, p, id)| (p, id)).collect()
    });
    for (p, id) in orphans {
        if chunk_loaded(game, &dimension, p) {
            kill_tagged(game, id);
            with(|s| s.dirty = true);
        } else {
            with(|s| s.orphans.push((dimension.clone(), p, id)));
        }
    }
    let ids: Vec<i64> = with(|s| s.graves.iter().filter(|g| g.dimension == dimension).map(|g| g.id).collect());
    for id in ids {
        let Some(g) = with(|s| s.graves.iter().find(|g| g.id == id).cloned()) else {
            continue;
        };
        if !chunk_loaded(game, &dimension, g.pos) {
            continue;
        }
        if g.removed {
            // Gone while away: the block and the model go now.
            remove_model(game, &g);
            restore_block(game, &g);
            with(|s| s.graves.retain(|x| x.id != id));
            continue;
        }
        // A grave whose block someone else replaced (a command, a structure) is gone.
        if game.native_block(g.pos) != GRAVE_BLOCK {
            with(|s| {
                if let Some(i) = grave_index(s, id) {
                    s.graves[i].removed = true;
                    s.dirty = true;
                }
            });
            remove_model(game, &g);
            continue;
        }
        if !g.visual {
            if try_break(game, id, None) {
                continue;
            }
            // `actively_move_inside_world_border`.
            let c = cfg();
            if c.actively_inside_border
                && c.inside_border
                && game.native_world_border(&dimension).is_some_and(|b| {
                    let (x, z) = (f64::from(g.pos[0]), f64::from(g.pos[2]));
                    x < b[0] || x >= b[2] || z < b[1] || z >= b[3]
                })
                && let (Some(to), _) = find_grave_position(game, &dimension, g.pos)
            {
                move_grave(game, id, &dimension, to);
            }
            protection_message(game, id);
        }
        refresh_model(game, id);
    }
    with(|s| s.graves.retain(|g| !(g.removed && g.dimension == dimension && g.shown.is_empty() && g.items.is_empty())));
}

/// `utilProtectionChangeMessage`: the owner learns that the protection ended.
fn protection_message(game: &mut Game<'_>, id: i64) {
    let Some(g) = with(|s| {
        let i = grave_index(s, id)?;
        let g = &mut s.graves[i];
        if g.protection_message_sent {
            return None;
        }
        let protected = s.graves[i].is_protected(s);
        let g = &mut s.graves[i];
        if protected {
            return None;
        }
        g.protection_message_sent = true;
        Some(g.clone())
    }) else {
        return;
    };
    let w = cfg().protection_ended.clone();
    let Some(owner) = g.owner.as_ref().map(|o| o.uuid).filter(|o| guest_index(game, *o).is_some()) else {
        return;
    };
    if !w.is_empty() {
        let lang = language(game, owner);
        let v = with(|s| w.with(&lang, &map_lookup(placeholders(&g, s, &lang))));
        tell(game, owner, v);
    }
}

fn act(game: &mut Game<'_>, a: Action) {
    let c = cfg();
    match a {
        Action::Use { uuid, dimension, pos, sneaking } => {
            let Some((id, access, visual)) =
                with(|s| grave_at(s, &dimension, pos).map(|i| (s.graves[i].id, s.graves[i].has_access(uuid, s), s.graves[i].visual)))
            else {
                return;
            };
            if visual {
                return;
            }
            let bypass = guest_index(game, uuid).is_some_and(|gi| {
                let g = &game.lan.as_ref().unwrap().hub.guests[gi];
                g.game_mode == 1 && g.permission >= 3
            });
            if !(access || bypass) {
                return;
            }
            if try_break(game, id, Some(uuid)) {
                return;
            }
            if c.shift_quick_pickup && (sneaking || !c.click_opens_gui) {
                quick_equip(game, uuid, id);
            } else if !sneaking {
                open_gui(game, uuid, Kind::Grave { id, previous: None }, true, false);
            }
        }
        Action::Break { uuid, dimension, pos } => {
            let Some(i) = with(|s| grave_at(s, &dimension, pos)) else {
                return;
            };
            let (id, visual, g) = with(|s| (s.graves[i].id, s.graves[i].visual, s.graves[i].clone()));
            if visual {
                // A gravestone breaks for anyone (`VisualGraveBlock`).
                with(|s| {
                    if let Some(i) = grave_index(s, id) {
                        s.graves[i].removed = true;
                        s.dirty = true;
                    }
                });
                remove_model(game, &g);
                if let Some(air) = game.native_default_state("minecraft:air") {
                    let _ = game.native_set_block(&dimension, pos, air, 3);
                }
                return;
            }
            let r = with(|s| rights(game, &g, uuid, s));
            if !r.can_take {
                return;
            }
            if c.break_quick_pickup && g.is_owner(uuid) {
                quick_equip(game, uuid, id);
            }
            destroy_grave(game, id, Some(uuid));
        }
        Action::Compass { uuid, grave } => {
            if with(|s| grave_index(s, grave).is_some()) {
                open_gui(game, uuid, Kind::Grave { id: grave, previous: None }, false, false);
            }
        }
        Action::Click { uuid, window, slot } => click(game, uuid, window, slot),
        Action::Closed { uuid, window } => {
            with(|s| {
                if s.guis.get(&uuid).is_some_and(|g| g.window == window) {
                    s.guis.remove(&uuid);
                }
            });
        }
        Action::Command { uuid, permission, args } => command(game, uuid, permission, &args),
    }
}

fn command(game: &mut Game<'_>, uuid: u128, permission: u8, args: &[String]) {
    let words: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
    let lang = language(game, uuid);
    let name = game.lan.as_ref().and_then(|h| h.hub.guests.iter().find(|g| g.uuid == uuid)).map(|g| g.name.clone()).unwrap_or_default();
    match words.as_slice() {
        [] => open_gui(game, uuid, Kind::List { target: uuid, name }, false, permission >= 3),
        ["modify"] if permission >= 3 => open_gui(game, uuid, Kind::List { target: uuid, name }, true, permission >= 3),
        ["player", who, rest @ ..] if permission >= 3 => {
            let modify = rest.first() == Some(&"modify");
            let target = game
                .lan
                .as_ref()
                .and_then(|h| h.hub.guests.iter().find(|g| g.name.eq_ignore_ascii_case(who)))
                .map(|g| (g.uuid, g.name.clone()))
                .or_else(|| {
                    with(|s| {
                        s.graves
                            .iter()
                            .filter_map(|g| g.owner.as_ref())
                            .find(|o| o.name.eq_ignore_ascii_case(who))
                            .map(|o| (o.uuid, o.name.clone()))
                    })
                });
            match target {
                Some((t, n)) => open_gui(game, uuid, Kind::List { target: t, name: n }, modify, modify && permission >= 3),
                None => tell(game, uuid, text::literal("This player doesn't exist!")),
            }
        }
        ["all", rest @ ..] if permission >= 3 => {
            let modify = rest.first() == Some(&"modify");
            open_gui(game, uuid, Kind::All, modify, modify && permission >= 3);
        }
        ["about"] => {
            for line in [
                json!({ "text": NAME, "color": "#ffd257", "bold": true }),
                text::translatable(&lang, "text.graves.about.version", vec![json!({ "text": format!("{VERSION} ({ORIGINAL})"), "color": "white" })]),
                json!({ "text": "", "color": "gray", "extra": [
                    { "text": "Patbox", "color": "white" },
                    { "text": " (Universal Graves, LGPL-3.0)\n" },
                    { "text": "DispatchedToHeaven", "color": "white" },
                    { "text": " (the Horizon port)" },
                ] }),
            ] {
                tell(game, uuid, line);
            }
        }
        ["reload"] if permission >= 4 => {
            if load_config() {
                tell(game, uuid, text::literal("Reloaded config!"));
            } else {
                tell(game, uuid, json!({ "text": "Error occurred while reloading config!", "color": "red" }));
            }
        }
        _ => {}
    }
}

/// `initInventoryMenu`: a respawned player with a grave gets its compass.
fn give_compass(game: &mut Game<'_>, uuid: u128) {
    if !cfg().give_compass {
        return;
    }
    let mut d = player_data(game, uuid);
    let last = get_i64(&d, "LastGraveId").unwrap_or(-1);
    if last == -1 || get_bool(&d, "HasGraveCompass").unwrap_or(false) {
        return;
    }
    let Some(grave) = with(|s| grave_index(s, last).map(|i| s.graves[i].clone())) else {
        return;
    };
    let lang = language(game, uuid);
    if let Some(stack) = compass(game, &grave, &lang, false) {
        give(game, uuid, stack);
        d.insert(jkey("HasGraveCompass"), Tag::Byte(1));
        set_player_data(game, uuid, d);
    }
}

/// `GraveCompassItem.inventoryTick`: a compass whose grave is gone goes (a
/// vanilla compass in its place when it was made from one).
fn check_compasses(game: &mut Game<'_>) {
    let Some(host) = game.lan.as_ref() else {
        return;
    };
    let mut found: Vec<(usize, usize, i64, u32)> = Vec::new();
    for (gi, g) in host.hub.guests.iter().enumerate() {
        for (k, s) in g.inventory.iter().enumerate() {
            if let Some(s) = s
                && let Some(id) = compass_grave(s)
            {
                found.push((gi, k, id, s.count));
            }
        }
    }
    for (gi, k, id, count) in found {
        if with(|s| grave_index(s, id).is_some()) {
            continue;
        }
        let host = game.lan.as_mut().unwrap();
        let g = &mut host.hub.guests[gi];
        let to_vanilla = g.inventory[k].as_ref().is_some_and(|s| {
            let Tag::Compound(c) = horizon_items::item_nbt::item_to_nbt(s) else {
                return false;
            };
            match c.get(&jkey("components")) {
                Some(Tag::Compound(comps)) => match comps.get(&jkey("minecraft:custom_data")) {
                    Some(Tag::Compound(d)) => match d.get(&jkey(COMPASS_KEY)) {
                        Some(Tag::Compound(m)) => get_bool(m, "to_vanilla").unwrap_or(false),
                        _ => false,
                    },
                    _ => false,
                },
                _ => false,
            }
        });
        g.inventory[k] = to_vanilla.then(|| ItemStack::new("minecraft:compass", count));
        host.hub.send_inventory(gi);
    }
}

// ---- Accessories' slot rules --------------------------------------------------------------

pub(super) fn add_reload_listeners(listeners: &mut Vec<super::native_reload_listeners::NativeReloadListener>) {
    listeners.push(super::native_reload_listeners::NativeReloadListener::json("universalgraves:accessories_slots", "accessories/slot"));
}

pub(super) fn reload_apply(listener: &super::native_reload_listeners::NativeReloadListener, files: &[(String, Value)]) {
    if listener.id != "universalgraves:accessories_slots" {
        return;
    }
    let mut rules = HashMap::new();
    for (id, json) in files {
        let path = id.split_once(':').map_or(id.as_str(), |(_, p)| p);
        let name = match path.split_once('/') {
            Some((ns, rest)) => format!("{ns}:{rest}"),
            None => path.to_owned(),
        };
        if let Some(rule) = json.get("dropRule").and_then(Value::as_str) {
            rules.insert(name, rule.to_owned());
        }
    }
    with(|s| s.accessory_rules = rules);
}

// ---- the mod list and Mod Menu ------------------------------------------------------------

thread_local! {
    static DATA_WRITTEN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn russian() -> bool {
    horizon_client::lang::code().starts_with("ru")
}

pub(super) fn list(mods: &mut Vec<super::native_hooks::NativeMod>) {
    mods.push(super::native_hooks::NativeMod {
        id: ID.into(),
        title: format!("{NAME} {VERSION}"),
        description: if russian() {
            "Могила с вещами и опытом там, где умер игрок.".into()
        } else {
            "A grave with your items and experience where you died.".into()
        },
        keys: Vec::new(),
    });
}

/// Mod Menu's icon and metadata in the mod's folder, at the first frame.
pub(super) fn frame(game: &Game<'_>) {
    if DATA_WRITTEN.with(|w| w.replace(true)) {
        return;
    }
    const ICON: &[u8] = horizon_client::native_data!("universalgraves", "icon.png");
    let dir = game.native_data_dir(ID);
    let _ = std::fs::create_dir_all(&dir);
    let json = format!(
        r#"{{
  "schema": 1,
  "name": "{NAME}",
  "version": "{VERSION}",
  "description": "A grave with your items and experience where you died, protected from other players for a while. Works on the server alone.",
  "description_ru": "Могила с вещами и опытом там, где умер игрок; какое-то время защищена от других игроков. Работает на одном сервере.",
  "authors": ["Patbox"],
  "contributors": ["DispatchedToHeaven (Horizon port)"],
  "license": "LGPL-3.0-only",
  "links": {{
    "homepage": "https://modrinth.com/mod/universal-graves",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/universalgraves",
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
