// First-person Model for Horizon: the original's settings
// (`FirstPersonSettings`, kept in `config/firstperson.json` as Gson writes
// them), the toggle (`FirstPersonBase.enabled`, F6) and `LogicHandler`'s
// decisions, apart from the game. Spliced into game.rs as `firstpersonmodel`.
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

pub const ID: &str = "firstpersonmodel";
/// The original's name (`fabric.mod.json`).
pub const NAME: &str = "FirstPerson";
pub const VERSION: &str = "1.0.0";
/// The original's file, in the game's `config/` folder.
pub const FILE: &str = "firstperson.json";
/// `Constants`.
pub const SNEAK_BODY_OFFSET: f32 = 0.27;
pub const SWIM_UP_BODY_OFFSET: f32 = 0.60;
pub const SWIM_DOWN_BODY_OFFSET: f32 = 0.50;
pub const IN_VEHICLE_BODY_OFFSET: f32 = 0.20;
/// The toggle key (`key.firstperson.toggle`, GLFW 295).
pub const TOGGLE_KEY: (&str, &str) = ("toggle", "key.keyboard.f6");

/// `VanillaHands`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VanillaHands {
    Off,
    All,
    AllDouble,
    Items,
}

impl VanillaHands {
    pub const ALL: [VanillaHands; 4] = [Self::Off, Self::All, Self::AllDouble, Self::Items];
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::All => "ALL",
            Self::AllDouble => "ALL_DOUBLE",
            Self::Items => "ITEMS",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|v| v.name() == s)
    }
    /// The next value (the enum button's click).
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// `FirstPersonSettings`, its fields in the original's order.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub config_version: i64,
    pub enabled_by_default: bool,
    pub x_offset: i32,
    pub sneak_x_offset: i32,
    pub sit_x_offset: i32,
    pub render_stuck_features: bool,
    pub vanilla_hands_mode: VanillaHands,
    pub dynamic_mode: bool,
    pub vanilla_hands_skip_swimming: bool,
    pub auto_vanilla_hands: Vec<String>,
    pub auto_toggle_mod_items: Vec<String>,
    pub hidden_layers: Vec<String>,
}

/// The default `autoVanillaHands` (and what `ConfigUpgrader` adds for version 1).
const AUTO_VANILLA_HANDS: [&str; 8] = [
    "antiqueatlas:antique_atlas",
    "twilightforest:filled_magic_map",
    "twilightforest:filled_maze_map",
    "twilightforest:filled_ore_map",
    "create:potato_cannon",
    "create:extendo_grip",
    "create:handheld_worldshaper",
    "map_atlases:atlas",
];

impl Default for Settings {
    fn default() -> Self {
        Self {
            config_version: 2,
            enabled_by_default: true,
            x_offset: 0,
            sneak_x_offset: 0,
            sit_x_offset: 0,
            render_stuck_features: true,
            vanilla_hands_mode: VanillaHands::Off,
            dynamic_mode: true,
            vanilla_hands_skip_swimming: true,
            auto_vanilla_hands: AUTO_VANILLA_HANDS.map(String::from).to_vec(),
            auto_toggle_mod_items: vec!["exposure:camera".into()],
            hidden_layers: Vec::new(),
        }
    }
}

/// A `Set<String>`'s add: kept once, in the order added.
pub fn add(set: &mut Vec<String>, value: &str) {
    if !set.iter().any(|v| v == value) {
        set.push(value.to_owned());
    }
}

pub fn remove(set: &mut Vec<String>, value: &str) {
    set.retain(|v| v != value);
}

impl Settings {
    /// Gson's `fromJson` over the defaults: a field the file lacks keeps its
    /// default, one of the wrong type fails the whole file (`None`).
    fn from_json(text: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        let object = value.as_object()?;
        let mut s = Self::default();
        let int = |key: &str, into: &mut i32| -> Option<()> {
            if let Some(v) = object.get(key) {
                *into = i32::try_from(v.as_i64()?).ok()?;
            }
            Some(())
        };
        let boolean = |key: &str, into: &mut bool| -> Option<()> {
            if let Some(v) = object.get(key) {
                *into = v.as_bool()?;
            }
            Some(())
        };
        let set = |key: &str, into: &mut Vec<String>| -> Option<()> {
            if let Some(v) = object.get(key) {
                let mut list = Vec::new();
                for item in v.as_array()? {
                    add(&mut list, item.as_str()?);
                }
                *into = list;
            }
            Some(())
        };
        if let Some(v) = object.get("configVersion") {
            s.config_version = v.as_i64()?;
        }
        boolean("enabledByDefault", &mut s.enabled_by_default)?;
        int("xOffset", &mut s.x_offset)?;
        int("sneakXOffset", &mut s.sneak_x_offset)?;
        int("sitXOffset", &mut s.sit_x_offset)?;
        boolean("renderStuckFeatures", &mut s.render_stuck_features)?;
        if let Some(v) = object.get("vanillaHandsMode") {
            // Gson reads an unknown constant as null: the field keeps no value.
            s.vanilla_hands_mode = VanillaHands::parse(v.as_str()?).unwrap_or(VanillaHands::Off);
        }
        boolean("dynamicMode", &mut s.dynamic_mode)?;
        boolean("vanillaHandsSkipSwimming", &mut s.vanilla_hands_skip_swimming)?;
        set("autoVanillaHands", &mut s.auto_vanilla_hands)?;
        set("autoToggleModItems", &mut s.auto_toggle_mod_items)?;
        set("hiddenLayers", &mut s.hidden_layers)?;
        Some(s)
    }

    /// `ConfigUpgrader.upgradeConfig`.
    fn upgrade(&mut self) {
        if self.config_version < 1 {
            for id in AUTO_VANILLA_HANDS {
                add(&mut self.auto_vanilla_hands, id);
            }
            self.config_version = 1;
        }
        if self.config_version < 2 {
            add(&mut self.auto_toggle_mod_items, "exposure:camera");
            self.config_version = 2;
        }
    }

    /// `new GsonBuilder().setPrettyPrinting().create().toJson(config)`.
    pub fn to_json(&self) -> String {
        fn string(s: &str) -> String {
            serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
        }
        fn set(values: &[String]) -> String {
            if values.is_empty() {
                return "[]".into();
            }
            let lines: Vec<String> = values.iter().map(|v| format!("    {}", string(v))).collect();
            format!("[\n{}\n  ]", lines.join(",\n"))
        }
        format!(
            "{{\n  \"configVersion\": {},\n  \"enabledByDefault\": {},\n  \"xOffset\": {},\n  \"sneakXOffset\": {},\n  \"sitXOffset\": {},\n  \"renderStuckFeatures\": {},\n  \"vanillaHandsMode\": {},\n  \"dynamicMode\": {},\n  \"vanillaHandsSkipSwimming\": {},\n  \"autoVanillaHands\": {},\n  \"autoToggleModItems\": {},\n  \"hiddenLayers\": {}\n}}",
            self.config_version,
            self.enabled_by_default,
            self.x_offset,
            self.sneak_x_offset,
            self.sit_x_offset,
            self.render_stuck_features,
            string(self.vanilla_hands_mode.name()),
            self.dynamic_mode,
            self.vanilla_hands_skip_swimming,
            set(&self.auto_vanilla_hands),
            set(&self.auto_toggle_mod_items),
            set(&self.hidden_layers),
        )
    }

    fn holds(set: &[String], item: Option<&str>) -> bool {
        item.is_some_and(|i| set.iter().any(|s| s == i))
    }

    /// An item of `autoVanillaHands` or `autoToggleModItems` in either hand.
    fn auto_item(&self, main: Option<&str>, off: Option<&str>) -> bool {
        Self::holds(&self.auto_vanilla_hands, main)
            || Self::holds(&self.auto_vanilla_hands, off)
            || Self::holds(&self.auto_toggle_mod_items, main)
            || Self::holds(&self.auto_toggle_mod_items, off)
    }

    /// An item of `autoToggleModItems` in either hand: the mod steps aside.
    pub fn disable_item(&self, main: Option<&str>, off: Option<&str>) -> bool {
        Self::holds(&self.auto_toggle_mod_items, main) || Self::holds(&self.auto_toggle_mod_items, off)
    }

    pub fn layer_hidden(&self, layer: &str) -> bool {
        self.hidden_layers.iter().any(|l| l == layer)
    }
}

/// What `LogicHandler` reads of the local player.
#[derive(Clone, Copy, Debug)]
pub struct Hands<'a> {
    pub main: Option<&'a str>,
    pub off: Option<&'a str>,
    /// `Entity.isSwimming`.
    pub swimming: bool,
    /// `getXRot()`.
    pub pitch: f32,
}

impl Settings {
    /// `LogicHandler.showVanillaHands`.
    pub fn show_vanilla_hands(&self, h: Hands<'_>) -> bool {
        matches!(self.vanilla_hands_mode, VanillaHands::All | VanillaHands::AllDouble)
            || (self.vanilla_hands_mode == VanillaHands::Items && (h.main.is_some() || h.off.is_some()))
            || self.auto_item(h.main, h.off)
    }

    /// `LogicHandler.dynamicHandsEnabled`.
    pub fn dynamic_hands_enabled(&self, h: Hands<'_>) -> bool {
        if self.vanilla_hands_skip_swimming && h.swimming {
            return false;
        }
        self.dynamic_mode && self.vanilla_hands_mode != VanillaHands::Off && !self.auto_item(h.main, h.off)
    }

    /// `LogicHandler.lookingDown`.
    pub fn looking_down(&self, h: Hands<'_>) -> bool {
        self.dynamic_hands_enabled(h) && h.pitch > 30.0
    }

    /// `LogicHandler.hideArmsAndItems`.
    pub fn hide_arms_and_items(&self, h: Hands<'_>) -> bool {
        if self.vanilla_hands_skip_swimming && h.swimming {
            return false;
        }
        if self.looking_down(h) {
            return false;
        }
        self.vanilla_hands_mode != VanillaHands::Off || self.auto_item(h.main, h.off)
    }

    /// `PlayerRendererMixin.extractRenderState`: what the own body hides in
    /// first person, and the arms' tilt. `main_map` is a filled map in the
    /// main hand.
    pub fn body_flags(&self, h: Hands<'_>, main_map: bool) -> horizon_client::mobs::anim::firstpersonmodel_pose::Flags {
        let mut f = horizon_client::mobs::anim::firstpersonmodel_pose::Flags::default();
        if self.hide_arms_and_items(h) {
            f.hide_left_arm = true;
            f.hide_right_arm = true;
        } else if self.dynamic_hands_enabled(h) {
            f.arm_offset = (-h.pitch / 20.0 + 2.0).clamp(0.0, 0.7);
            if !self.looking_down(h) {
                if h.off.is_some() || main_map {
                    f.hide_left_arm = true;
                }
                if h.main.is_some() {
                    f.hide_right_arm = true;
                }
            }
        }
        if h.swimming {
            f.hide_body = true;
        }
        f
    }
}

/// `Mth.wrapDegrees`.
pub fn wrap_degrees(d: f32) -> f32 {
    let mut d = d % 360.0;
    if d >= 180.0 {
        d -= 360.0;
    }
    if d < -180.0 {
        d += 360.0;
    }
    d
}

/// `Mth.rotLerp`.
pub fn rot_lerp(delta: f32, from: f32, to: f32) -> f32 {
    from + delta * wrap_degrees(to - from)
}

/// `LogicHandler.calculateBodyRot`: a rider's body on a living vehicle.
pub fn calculate_body_rot(entity_body_rot: f32, rider_head_rot: f32) -> f32 {
    let wrapped_head = wrap_degrees(rider_head_rot);
    let diff = wrap_degrees(wrapped_head - entity_body_rot);
    let mut body = entity_body_rot;
    if diff.abs() > 50.0 {
        body = wrapped_head - 50.0 * diff.signum();
    }
    wrap_degrees(body)
}

/// The mod's state between frames.
pub struct State {
    pub settings: Settings,
    pub loaded: bool,
    /// `FirstPersonBase.enabled`, the F6 toggle.
    pub enabled: bool,
    /// The toggle key is held (`isHeld`).
    pub held: bool,
    /// `LogicHandler.timeout`: the activation handler's 100 ms after
    /// sleeping, gliding, a spin attack or half-swimming.
    pub timeout: Option<Instant>,
}

static STATE: Mutex<State> = Mutex::new(State {
    settings: Settings {
        config_version: 2,
        enabled_by_default: true,
        x_offset: 0,
        sneak_x_offset: 0,
        sit_x_offset: 0,
        render_stuck_features: true,
        vanilla_hands_mode: VanillaHands::Off,
        dynamic_mode: true,
        vanilla_hands_skip_swimming: true,
        auto_vanilla_hands: Vec::new(),
        auto_toggle_mod_items: Vec::new(),
        hidden_layers: Vec::new(),
    },
    loaded: false,
    enabled: true,
    held: false,
    timeout: None,
});

pub fn state() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

/// A copy of the settings now.
pub fn settings() -> Settings {
    state().settings.clone()
}

fn file() -> Option<std::path::PathBuf> {
    super::Game::native_config_dir().ok().map(|d| d.join(FILE))
}

/// `FirstPersonBase.loadConfig`: the file, else the defaults; upgraded,
/// written back, and the toggle set to `enabledByDefault`.
pub fn load() {
    let mut settings = file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|text| Settings::from_json(&text))
        .unwrap_or_default();
    settings.upgrade();
    let mut s = state();
    s.enabled = settings.enabled_by_default;
    s.settings = settings;
    s.loaded = true;
    let json = s.settings.to_json();
    drop(s);
    write(&json);
}

fn write(json: &str) {
    if let Some(f) = file() {
        let _ = std::fs::write(f, json);
    }
}

/// `FirstPersonBase.writeSettings`.
pub fn save() {
    let json = state().settings.to_json();
    write(&json);
}

/// Changes the settings (not yet written).
pub fn edit(f: impl FnOnce(&mut Settings)) {
    f(&mut state().settings);
}

/// `FirstPersonBase.resetSettings`.
pub fn reset() {
    state().settings = Settings::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_round_trips_as_gson_writes_it() {
        let s = Settings::default();
        assert_eq!(Settings::from_json(&s.to_json()), Some(s));
    }

    #[test]
    fn an_old_file_is_upgraded() {
        let mut s = Settings::from_json(r#"{"configVersion": 0, "xOffset": 5}"#).unwrap();
        s.auto_toggle_mod_items.clear();
        s.upgrade();
        assert_eq!(s.x_offset, 5);
        assert_eq!(s.config_version, 2);
        assert_eq!(s.auto_toggle_mod_items, vec!["exposure:camera".to_string()]);
    }
}
