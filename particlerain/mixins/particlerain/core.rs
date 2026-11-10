// Particle Rain's settings and particles: `ConfigData` (config/particlerain/config.json),
// `ParticleData` with its enums, the built-in particles (`particles.json`) merged with the
// player's own (config/particlerain/particles.json, only what differs, as `ParticleLoader` keeps
// it), the block and biome lists (`Whitelist`) and the tags they name. Spliced into game.rs as
// `particlerain_core`.
//
// Particle Rain by PigCart (https://github.com/PigCart/particle-rain), MIT.

use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub(super) const ID: &str = "particlerain";
pub(super) const NAME: &str = "Particle Rain";
/// The port's version (the original's is 4.0.3).
pub(super) const VERSION: &str = "1.0.0";
/// `ConfigData.configVersion`: an older file is replaced by the defaults.
const CONFIG_VERSION: i64 = 8;

// ---- enums ---------------------------------------------------------------------------------

/// An enum the screen cycles through: its class name (the translation key's middle,
/// `particlerain.<Class>.<VALUE>`) and its values in declaration order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum EnumKind {
    ParticleStyle,
    Weather,
    Precipitation,
    SpawnPos,
    FadeType,
    RenderType,
    TintType,
    RotationType,
}

impl EnumKind {
    pub(super) fn class(self) -> &'static str {
        match self {
            EnumKind::ParticleStyle => "ParticleStyle",
            EnumKind::Weather => "Weather",
            EnumKind::Precipitation => "Precipitation",
            EnumKind::SpawnPos => "SpawnPos",
            EnumKind::FadeType => "FadeType",
            EnumKind::RenderType => "RenderType",
            EnumKind::TintType => "TintType",
            EnumKind::RotationType => "RotationType",
        }
    }
    pub(super) fn values(self) -> &'static [&'static str] {
        match self {
            EnumKind::ParticleStyle => &["REGISTERED", "BLOCK_MODEL", "CUSTOM"],
            EnumKind::Weather => &["DURING_WEATHER", "ONLY_DURING_NORMAL_WEATHER", "ONLY_DURING_STORMY_WEATHER", "AFTER_WEATHER", "CLEAR", "ALWAYS"],
            EnumKind::Precipitation => &["NONE", "RAIN", "SNOW"],
            EnumKind::SpawnPos => &["SKY", "BLOCK_SIDES", "BLOCK_BOTTOM", "BLOCK_TOP", "WORLD_SURFACE"],
            EnumKind::FadeType => &["IN_AND_OUT", "OUT", "DISTANCE"],
            EnumKind::RenderType => &["OPAQUE", "TRANSLUCENT", "BLENDED"],
            EnumKind::TintType => &["WATER", "FOG", "MAP", "CUSTOM", "NONE"],
            EnumKind::RotationType => &["COPY_CAMERA", "RELATIVE_VELOCITY", "WORLD_VELOCITY", "VERTICAL", "HORIZONTAL"],
        }
    }
    pub(super) fn index(self, name: &str) -> Option<u8> {
        self.values().iter().position(|v| *v == name).map(|i| i as u8)
    }
}

macro_rules! particlerain_enum {
    ($name:ident, $kind:expr, [$($variant:ident),*]) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub(super) enum $name { $($variant),* }
        impl $name {
            const ALL: &'static [$name] = &[$($name::$variant),*];
            pub(super) fn index(self) -> u8 {
                Self::ALL.iter().position(|v| *v == self).unwrap_or(0) as u8
            }
            pub(super) fn from_index(i: u8) -> Self {
                Self::ALL.get(usize::from(i)).copied().unwrap_or(Self::ALL[0])
            }
            pub(super) fn name(self) -> &'static str {
                $kind.values()[usize::from(self.index())]
            }
            fn parse(v: &Value) -> Option<Self> {
                $kind.index(v.as_str()?).map(Self::from_index)
            }
        }
    };
}

particlerain_enum!(ParticleStyle, EnumKind::ParticleStyle, [Registered, BlockModel, Custom]);
particlerain_enum!(Weather, EnumKind::Weather, [DuringWeather, OnlyDuringNormalWeather, OnlyDuringStormyWeather, AfterWeather, Clear, Always]);
particlerain_enum!(Precipitation, EnumKind::Precipitation, [None, Rain, Snow]);
particlerain_enum!(SpawnPos, EnumKind::SpawnPos, [Sky, BlockSides, BlockBottom, BlockTop, WorldSurface]);
particlerain_enum!(FadeType, EnumKind::FadeType, [InAndOut, Out, Distance]);
particlerain_enum!(RenderType, EnumKind::RenderType, [Opaque, Translucent, Blended]);
particlerain_enum!(TintType, EnumKind::TintType, [Water, Fog, Map, Custom, None]);
particlerain_enum!(RotationType, EnumKind::RotationType, [CopyCamera, RelativeVelocity, WorldVelocity, Vertical, Horizontal]);

impl Weather {
    /// `Weather.isCurrent`: the rain and thunder levels at a whole tick, and whether the
    /// "after weather" time is running.
    pub(super) fn is_current(self, rain: f32, thunder: f32, after_weather: bool) -> bool {
        match self {
            Weather::DuringWeather => rain > 0.0,
            Weather::OnlyDuringNormalWeather => rain > 0.0 && thunder <= 0.0,
            Weather::OnlyDuringStormyWeather => thunder > 0.0,
            Weather::AfterWeather => after_weather,
            Weather::Clear => rain <= 0.0,
            Weather::Always => true,
        }
    }
}

// ---- lists ---------------------------------------------------------------------------------

/// `Whitelist`: block or biome ids and `#tags`, allowed (whitelist) or refused (blacklist).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Whitelist {
    pub(super) is_whitelist: bool,
    pub(super) entries: Vec<String>,
}

impl Whitelist {
    pub(super) fn new(is_whitelist: bool, entries: &[&str]) -> Self {
        Whitelist { is_whitelist, entries: entries.iter().map(|s| (*s).to_owned()).collect() }
    }
    /// `contains`: tags first, then ids; the first match decides; no match allows a blacklist's
    /// entry and refuses a whitelist's; an empty list allows everything. `is` tells whether the
    /// block or biome is in a tag.
    pub(super) fn contains(&self, id: &str, is: &dyn Fn(&str) -> bool) -> bool {
        if self.entries.is_empty() {
            return true;
        }
        for e in &self.entries {
            if let Some(tag) = e.strip_prefix('#')
                && let Some(tag) = parse_id(tag)
                && is(&tag)
            {
                return self.is_whitelist;
            }
        }
        for e in &self.entries {
            if !e.starts_with('#')
                && let Some(i) = parse_id(e)
                && i == id
            {
                return self.is_whitelist;
            }
        }
        !self.is_whitelist
    }
    /// `toString`: the entries only (what `isolateCustomizedValues` compares).
    fn as_string(&self) -> String {
        format!("[{}]", self.entries.join(", "))
    }
    fn parse_onto(&mut self, v: &Value) {
        if let Some(b) = v.get("isWhitelist").and_then(Value::as_bool) {
            self.is_whitelist = b;
        }
        if let Some(a) = v.get("entries").and_then(Value::as_array) {
            self.entries = a.iter().filter_map(|e| e.as_str().map(str::to_owned)).collect();
        }
    }
    fn write(&self, w: &mut Json) {
        w.open('{');
        w.key("entries");
        w.strings(&self.entries);
        w.key("isWhitelist");
        w.raw(&self.is_whitelist.to_string());
        w.close('}');
    }
}

/// `ResourceLocation.parse`: `path` alone is `minecraft:path`; None for an invalid id.
pub(super) fn parse_id(s: &str) -> Option<String> {
    let (ns, path) = s.split_once(':').unwrap_or(("minecraft", s));
    let ns_ok = ns.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.'));
    let path_ok = path.chars().all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.' | '/'));
    (ns_ok && path_ok).then(|| format!("{ns}:{path}"))
}

/// A block or biome name with its namespace.
pub(super) fn full_id(name: &str) -> String {
    if name.contains(':') { name.to_owned() } else { format!("minecraft:{name}") }
}

// ---- the settings ----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Perf {
    pub(super) particle_amount: i32,
    pub(super) particle_storm_density: f32,
    pub(super) particle_distance: i32,
    pub(super) surface_range: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Sound {
    pub(super) rain_volume: f32,
    pub(super) snow_volume: f32,
    pub(super) wind_volume: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Wind {
    pub(super) strength: f32,
    pub(super) gust_frequency: f32,
    pub(super) modulation_speed: f32,
    pub(super) y_level_adjustment: bool,
    /// The port's addition: the wind turns slowly with the game time (off: the original's fixed
    /// north-west wind). Kept as `horizonChangingWindDirection`.
    pub(super) changing_direction: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Compat {
    pub(super) render_default_rain: bool,
    pub(super) render_default_snow: bool,
    pub(super) do_default_splashing: bool,
    pub(super) water_tint: bool,
    pub(super) tint_mix: f32,
    pub(super) shaderpack_tint: bool,
    pub(super) sync_registries: bool,
    pub(super) cross_biome_border: bool,
    pub(super) use_heightmap_temp: bool,
    pub(super) weather_ignore_blocks: Whitelist,
    pub(super) do_spawn_height_limit: bool,
    pub(super) spawn_height_limit: i32,
    pub(super) ripple_res: i32,
    pub(super) ripple_use_pack_res: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Streak {
    pub(super) opacity: f32,
    pub(super) size: f32,
}

/// `ConfigData`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Config {
    pub(super) perf: Perf,
    pub(super) sound: Sound,
    pub(super) wind: Wind,
    pub(super) compat: Compat,
    pub(super) streak: Streak,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            perf: Perf { particle_amount: 1500, particle_storm_density: 1.5, particle_distance: 16, surface_range: 100 },
            sound: Sound { rain_volume: 0.2, snow_volume: 0.1, wind_volume: 0.1 },
            wind: Wind { strength: 0.03, gust_frequency: 0.15, modulation_speed: 0.1, y_level_adjustment: true, changing_direction: true },
            compat: Compat {
                render_default_rain: false,
                render_default_snow: false,
                do_default_splashing: false,
                water_tint: true,
                tint_mix: 0.6,
                shaderpack_tint: true,
                sync_registries: true,
                cross_biome_border: false,
                use_heightmap_temp: true,
                weather_ignore_blocks: Whitelist::new(true, &["#minecraft:fences", "#minecraft:all_signs", "minecraft:barrier"]),
                do_spawn_height_limit: false,
                spawn_height_limit: 0,
                ripple_res: 16,
                ripple_use_pack_res: true,
            },
            streak: Streak { opacity: 0.9, size: 0.5 },
        }
    }
}

fn f32_of(v: &Value) -> Option<f32> {
    v.as_f64().map(|f| f as f32)
}
fn i32_of(v: &Value) -> Option<i32> {
    v.as_i64().map(|i| i as i32).or_else(|| v.as_f64().map(|f| f as i32))
}
fn set_f32(o: &Value, key: &str, to: &mut f32) {
    if let Some(v) = o.get(key).and_then(f32_of) {
        *to = v;
    }
}
fn set_i32(o: &Value, key: &str, to: &mut i32) {
    if let Some(v) = o.get(key).and_then(i32_of) {
        *to = v;
    }
}
fn set_bool(o: &Value, key: &str, to: &mut bool) {
    if let Some(v) = o.get(key).and_then(Value::as_bool) {
        *to = v;
    }
}
fn set_string(o: &Value, key: &str, to: &mut String) {
    if let Some(v) = o.get(key).and_then(Value::as_str) {
        *to = v.to_owned();
    }
}

impl Config {
    /// `ConfigManager.load`'s reading: Gson fills what the file has over the defaults. None when
    /// the file is not a config (or an older version's), which the defaults then replace.
    pub(super) fn parse(text: &str) -> Option<Config> {
        let v: Value = serde_json::from_str(text).ok()?;
        if !v.is_object() {
            return None;
        }
        let version = v.get("configVersion").and_then(Value::as_i64).unwrap_or(CONFIG_VERSION);
        if version < CONFIG_VERSION {
            return None;
        }
        let mut c = Config::default();
        if let Some(p) = v.get("perf") {
            set_i32(p, "particleAmount", &mut c.perf.particle_amount);
            set_f32(p, "particleStormDensity", &mut c.perf.particle_storm_density);
            set_i32(p, "particleDistance", &mut c.perf.particle_distance);
            set_i32(p, "surfaceRange", &mut c.perf.surface_range);
        }
        if let Some(s) = v.get("sound") {
            set_f32(s, "rainVolume", &mut c.sound.rain_volume);
            set_f32(s, "snowVolume", &mut c.sound.snow_volume);
            set_f32(s, "windVolume", &mut c.sound.wind_volume);
        }
        if let Some(w) = v.get("wind") {
            set_f32(w, "strength", &mut c.wind.strength);
            set_f32(w, "gustFrequency", &mut c.wind.gust_frequency);
            set_f32(w, "modulationSpeed", &mut c.wind.modulation_speed);
            set_bool(w, "yLevelAdjustment", &mut c.wind.y_level_adjustment);
            set_bool(w, "horizonChangingWindDirection", &mut c.wind.changing_direction);
        }
        if let Some(k) = v.get("compat") {
            let c = &mut c.compat;
            set_bool(k, "renderDefaultRain", &mut c.render_default_rain);
            set_bool(k, "renderDefaultSnow", &mut c.render_default_snow);
            set_bool(k, "doDefaultSplashing", &mut c.do_default_splashing);
            set_bool(k, "waterTint", &mut c.water_tint);
            set_f32(k, "tintMix", &mut c.tint_mix);
            set_bool(k, "shaderpackTint", &mut c.shaderpack_tint);
            set_bool(k, "syncRegistries", &mut c.sync_registries);
            set_bool(k, "crossBiomeBorder", &mut c.cross_biome_border);
            set_bool(k, "useHeightmapTemp", &mut c.use_heightmap_temp);
            if let Some(l) = k.get("weatherIgnoreBlocks") {
                c.weather_ignore_blocks.parse_onto(l);
            }
            set_bool(k, "doSpawnHeightLimit", &mut c.do_spawn_height_limit);
            set_i32(k, "spawnHeightLimit", &mut c.spawn_height_limit);
            set_i32(k, "rippleRes", &mut c.ripple_res);
            set_bool(k, "rippleUsePackRes", &mut c.ripple_use_pack_res);
        }
        if let Some(s) = v.get("streak") {
            set_f32(s, "opacity", &mut c.streak.opacity);
            set_f32(s, "size", &mut c.streak.size);
        }
        Some(c)
    }

    /// The file as Gson writes it (pretty, fields in declaration order).
    pub(super) fn to_json(&self) -> String {
        let mut w = Json::default();
        w.open('{');
        w.key("configVersion");
        w.raw(&CONFIG_VERSION.to_string());
        w.key("perf");
        w.open('{');
        w.int("particleAmount", self.perf.particle_amount);
        w.float("particleStormDensity", self.perf.particle_storm_density);
        w.int("particleDistance", self.perf.particle_distance);
        w.int("surfaceRange", self.perf.surface_range);
        w.close('}');
        w.key("sound");
        w.open('{');
        w.float("rainVolume", self.sound.rain_volume);
        w.float("snowVolume", self.sound.snow_volume);
        w.float("windVolume", self.sound.wind_volume);
        w.close('}');
        w.key("wind");
        w.open('{');
        w.float("strength", self.wind.strength);
        w.float("gustFrequency", self.wind.gust_frequency);
        w.float("modulationSpeed", self.wind.modulation_speed);
        w.boolean("yLevelAdjustment", self.wind.y_level_adjustment);
        w.boolean("horizonChangingWindDirection", self.wind.changing_direction);
        w.close('}');
        w.key("compat");
        w.open('{');
        let c = &self.compat;
        w.boolean("renderDefaultRain", c.render_default_rain);
        w.boolean("renderDefaultSnow", c.render_default_snow);
        w.boolean("doDefaultSplashing", c.do_default_splashing);
        w.boolean("waterTint", c.water_tint);
        w.float("tintMix", c.tint_mix);
        w.boolean("shaderpackTint", c.shaderpack_tint);
        w.boolean("syncRegistries", c.sync_registries);
        w.boolean("crossBiomeBorder", c.cross_biome_border);
        w.boolean("useHeightmapTemp", c.use_heightmap_temp);
        w.key("weatherIgnoreBlocks");
        c.weather_ignore_blocks.write(&mut w);
        w.boolean("doSpawnHeightLimit", c.do_spawn_height_limit);
        w.int("spawnHeightLimit", c.spawn_height_limit);
        w.int("rippleRes", c.ripple_res);
        w.boolean("rippleUsePackRes", c.ripple_use_pack_res);
        w.close('}');
        w.key("streak");
        w.open('{');
        w.float("opacity", self.streak.opacity);
        w.float("size", self.streak.size);
        w.close('}');
        w.close('}');
        w.finish()
    }
}

// ---- a particle -----------------------------------------------------------------------------

/// `ParticleData`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ParticleData {
    /// The key in `particles.json` (transient in Java).
    pub(super) id: String,
    pub(super) particle_style: ParticleStyle,
    pub(super) registered_particle_id: String,
    pub(super) block_id: String,
    pub(super) enabled: bool,
    pub(super) density: f32,
    pub(super) weather: Weather,
    pub(super) precipitation: Vec<Precipitation>,
    pub(super) biome_list: Whitelist,
    pub(super) block_list: Whitelist,
    pub(super) needs_sky_access: bool,
    pub(super) spawn_pos: SpawnPos,
    pub(super) wind_strength: f32,
    pub(super) storm_wind_strength: f32,
    pub(super) gravity: f32,
    pub(super) rotation_amount: f32,
    pub(super) bounciness: f32,
    pub(super) lifetime: i32,
    pub(super) opacity: f32,
    pub(super) fade_type: FadeType,
    pub(super) size: f32,
    pub(super) distance_scaling: f32,
    pub(super) render_type: RenderType,
    pub(super) animate_sprites: bool,
    pub(super) sprite_locations: Vec<String>,
    pub(super) tint_type: TintType,
    /// `customTint` as 0xRRGGBB.
    pub(super) custom_tint: u32,
    pub(super) rotation_type: RotationType,
}

impl Default for ParticleData {
    fn default() -> Self {
        ParticleData {
            id: "new_particle".into(),
            particle_style: ParticleStyle::Custom,
            registered_particle_id: "minecraft:cloud".into(),
            block_id: "minecraft:dead_bush".into(),
            enabled: true,
            density: 1.0,
            weather: Weather::DuringWeather,
            precipitation: vec![Precipitation::Rain],
            biome_list: Whitelist::new(true, &[]),
            block_list: Whitelist::new(true, &[]),
            needs_sky_access: true,
            spawn_pos: SpawnPos::Sky,
            wind_strength: 0.1,
            storm_wind_strength: 0.5,
            gravity: 0.1,
            rotation_amount: 0.0,
            bounciness: 0.0,
            lifetime: 3000,
            opacity: 1.0,
            fade_type: FadeType::Distance,
            size: 0.5,
            distance_scaling: 0.0,
            render_type: RenderType::Translucent,
            animate_sprites: false,
            sprite_locations: vec!["particlerain:new_custom_particle".into()],
            tint_type: TintType::None,
            custom_tint: 0,
            rotation_type: RotationType::CopyCamera,
        }
    }
}

/// `ColorTypeAdapter.getColor`: `Color.decode` of "#RRGGBB", "0xRRGGBB" or a number.
pub(super) fn decode_color(s: &str) -> Option<u32> {
    let s = s.trim();
    let (neg, s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let v = if let Some(h) = s.strip_prefix('#').or_else(|| s.strip_prefix("0x")).or_else(|| s.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else if s.len() > 1 && s.starts_with('0') {
        i64::from_str_radix(&s[1..], 8).ok()?
    } else {
        s.parse::<i64>().ok()?
    };
    let v = if neg { -v } else { v };
    i32::try_from(v).ok().map(|v| (v as u32) & 0xFF_FFFF)
}

/// `ColorTypeAdapter.getString`: "#RRGGBB".
pub(super) fn color_string(c: u32) -> String {
    format!("#{:06X}", c & 0xFF_FFFF)
}

/// Java's `Float.toString` for the values a setting holds (what the number fields show and the
/// file compares): the shortest text that reads back to the same float, ".0" for whole numbers,
/// scientific below 10^-3 and from 10^7.
pub(super) fn java_float(v: f32) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let a = v.abs();
    if (1.0e-3..1.0e7).contains(&a) {
        let s = format!("{v}");
        if s.contains('.') { s } else { format!("{s}.0") }
    } else {
        let s = format!("{v:e}");
        let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
        let m = if m.contains('.') { m.to_owned() } else { format!("{m}.0") };
        format!("{m}E{e}")
    }
}

/// One of `ParticleData`'s fields, in declaration order (the order Gson writes them and the
/// screen lists them).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Field {
    ParticleStyle,
    RegisteredParticleId,
    BlockId,
    Id,
    Enabled,
    Density,
    Weather,
    Precipitation,
    BiomeList,
    BlockList,
    NeedsSkyAccess,
    SpawnPos,
    WindStrength,
    StormWindStrength,
    Gravity,
    RotationAmount,
    Bounciness,
    Lifetime,
    Opacity,
    FadeType,
    Size,
    DistanceScaling,
    RenderType,
    AnimateSprites,
    SpriteLocations,
    TintType,
    CustomTint,
    RotationType,
}

impl Field {
    pub(super) const ALL: [Field; 28] = [
        Field::ParticleStyle,
        Field::RegisteredParticleId,
        Field::BlockId,
        Field::Id,
        Field::Enabled,
        Field::Density,
        Field::Weather,
        Field::Precipitation,
        Field::BiomeList,
        Field::BlockList,
        Field::NeedsSkyAccess,
        Field::SpawnPos,
        Field::WindStrength,
        Field::StormWindStrength,
        Field::Gravity,
        Field::RotationAmount,
        Field::Bounciness,
        Field::Lifetime,
        Field::Opacity,
        Field::FadeType,
        Field::Size,
        Field::DistanceScaling,
        Field::RenderType,
        Field::AnimateSprites,
        Field::SpriteLocations,
        Field::TintType,
        Field::CustomTint,
        Field::RotationType,
    ];
    /// The Java field's name: the JSON key and the translation key's end.
    pub(super) fn name(self) -> &'static str {
        match self {
            Field::ParticleStyle => "particleStyle",
            Field::RegisteredParticleId => "registeredParticleId",
            Field::BlockId => "blockId",
            Field::Id => "id",
            Field::Enabled => "enabled",
            Field::Density => "density",
            Field::Weather => "weather",
            Field::Precipitation => "precipitation",
            Field::BiomeList => "biomeList",
            Field::BlockList => "blockList",
            Field::NeedsSkyAccess => "needsSkyAccess",
            Field::SpawnPos => "spawnPos",
            Field::WindStrength => "windStrength",
            Field::StormWindStrength => "stormWindStrength",
            Field::Gravity => "gravity",
            Field::RotationAmount => "rotationAmount",
            Field::Bounciness => "bounciness",
            Field::Lifetime => "lifetime",
            Field::Opacity => "opacity",
            Field::FadeType => "fadeType",
            Field::Size => "size",
            Field::DistanceScaling => "distanceScaling",
            Field::RenderType => "renderType",
            Field::AnimateSprites => "animateSprites",
            Field::SpriteLocations => "spriteLocations",
            Field::TintType => "tintType",
            Field::CustomTint => "customTint",
            Field::RotationType => "rotationType",
        }
    }
}

impl ParticleData {
    /// Gson's reading of one particle over `self`: only the keys the object has.
    fn parse_onto(&mut self, o: &Value) {
        if let Some(v) = o.get("particleStyle").and_then(ParticleStyle::parse) {
            self.particle_style = v;
        }
        set_string(o, "registeredParticleId", &mut self.registered_particle_id);
        set_string(o, "blockId", &mut self.block_id);
        set_bool(o, "enabled", &mut self.enabled);
        set_f32(o, "density", &mut self.density);
        if let Some(v) = o.get("weather").and_then(Weather::parse) {
            self.weather = v;
        }
        if let Some(a) = o.get("precipitation").and_then(Value::as_array) {
            self.precipitation = a.iter().filter_map(Precipitation::parse).collect();
        }
        if let Some(l) = o.get("biomeList") {
            self.biome_list.parse_onto(l);
        }
        if let Some(l) = o.get("blockList") {
            self.block_list.parse_onto(l);
        }
        set_bool(o, "needsSkyAccess", &mut self.needs_sky_access);
        if let Some(v) = o.get("spawnPos").and_then(SpawnPos::parse) {
            self.spawn_pos = v;
        }
        set_f32(o, "windStrength", &mut self.wind_strength);
        set_f32(o, "stormWindStrength", &mut self.storm_wind_strength);
        set_f32(o, "gravity", &mut self.gravity);
        set_f32(o, "rotationAmount", &mut self.rotation_amount);
        set_f32(o, "bounciness", &mut self.bounciness);
        set_i32(o, "lifetime", &mut self.lifetime);
        set_f32(o, "opacity", &mut self.opacity);
        if let Some(v) = o.get("fadeType").and_then(FadeType::parse) {
            self.fade_type = v;
        }
        set_f32(o, "size", &mut self.size);
        set_f32(o, "distanceScaling", &mut self.distance_scaling);
        if let Some(v) = o.get("renderType").and_then(RenderType::parse) {
            self.render_type = v;
        }
        set_bool(o, "animateSprites", &mut self.animate_sprites);
        if let Some(a) = o.get("spriteLocations").and_then(Value::as_array) {
            self.sprite_locations = a.iter().filter_map(|e| e.as_str().map(str::to_owned)).collect();
        }
        if let Some(v) = o.get("tintType").and_then(TintType::parse) {
            self.tint_type = v;
        }
        if let Some(c) = o.get("customTint").and_then(Value::as_str).and_then(decode_color) {
            self.custom_tint = c;
        }
        if let Some(v) = o.get("rotationType").and_then(RotationType::parse) {
            self.rotation_type = v;
        }
    }

    /// A field's value as `toString` gives it (what `isolateCustomizedValues` compares).
    fn field_string(&self, f: Field) -> String {
        match f {
            Field::ParticleStyle => self.particle_style.name().into(),
            Field::RegisteredParticleId => self.registered_particle_id.clone(),
            Field::BlockId => self.block_id.clone(),
            Field::Id => self.id.clone(),
            Field::Enabled => self.enabled.to_string(),
            Field::Density => java_float(self.density),
            Field::Weather => self.weather.name().into(),
            Field::Precipitation => format!("[{}]", self.precipitation.iter().map(|p| p.name()).collect::<Vec<_>>().join(", ")),
            Field::BiomeList => self.biome_list.as_string(),
            Field::BlockList => self.block_list.as_string(),
            Field::NeedsSkyAccess => self.needs_sky_access.to_string(),
            Field::SpawnPos => self.spawn_pos.name().into(),
            Field::WindStrength => java_float(self.wind_strength),
            Field::StormWindStrength => java_float(self.storm_wind_strength),
            Field::Gravity => java_float(self.gravity),
            Field::RotationAmount => java_float(self.rotation_amount),
            Field::Bounciness => java_float(self.bounciness),
            Field::Lifetime => self.lifetime.to_string(),
            Field::Opacity => java_float(self.opacity),
            Field::FadeType => self.fade_type.name().into(),
            Field::Size => java_float(self.size),
            Field::DistanceScaling => java_float(self.distance_scaling),
            Field::RenderType => self.render_type.name().into(),
            Field::AnimateSprites => self.animate_sprites.to_string(),
            Field::SpriteLocations => format!("[{}]", self.sprite_locations.join(", ")),
            Field::TintType => self.tint_type.name().into(),
            Field::CustomTint => format!("java.awt.Color[{}]", self.custom_tint),
            Field::RotationType => self.rotation_type.name().into(),
        }
    }

    fn write_field(&self, f: Field, w: &mut Json) {
        w.key(f.name());
        match f {
            Field::Precipitation => w.strings(&self.precipitation.iter().map(|p| p.name().to_owned()).collect::<Vec<_>>()),
            Field::BiomeList => self.biome_list.write(w),
            Field::BlockList => self.block_list.write(w),
            Field::SpriteLocations => w.strings(&self.sprite_locations),
            Field::CustomTint => w.string(&color_string(self.custom_tint)),
            Field::ParticleStyle
            | Field::RegisteredParticleId
            | Field::BlockId
            | Field::Id
            | Field::Weather
            | Field::SpawnPos
            | Field::FadeType
            | Field::RenderType
            | Field::TintType
            | Field::RotationType => w.string(&self.field_string(f)),
            Field::Enabled | Field::NeedsSkyAccess | Field::AnimateSprites | Field::Lifetime => w.raw(&self.field_string(f)),
            _ => w.raw(&json_float(self.float(f))),
        }
    }

    fn float(&self, f: Field) -> f32 {
        match f {
            Field::Density => self.density,
            Field::WindStrength => self.wind_strength,
            Field::StormWindStrength => self.storm_wind_strength,
            Field::Gravity => self.gravity,
            Field::RotationAmount => self.rotation_amount,
            Field::Bounciness => self.bounciness,
            Field::Opacity => self.opacity,
            Field::Size => self.size,
            Field::DistanceScaling => self.distance_scaling,
            _ => 0.0,
        }
    }

    /// `isolateCustomizedValues` written as Gson writes it: against the built-in particle, only
    /// the fields whose text differs (`{}` when none); a particle of the player's own whole.
    fn write_saved(&self, default: Option<&ParticleData>, w: &mut Json) {
        w.open('{');
        for f in Field::ALL {
            if f == Field::Id {
                continue;
            }
            if let Some(d) = default
                && d.field_string(f) == self.field_string(f)
            {
                continue;
            }
            self.write_field(f, w);
        }
        w.close('}');
    }
}

/// Gson's text for a float: `Float.toString`.
fn json_float(v: f32) -> String {
    if v.is_finite() { java_float(v) } else { "0.0".into() }
}

/// `deserializeParticles`: each particle of a `particles.json` merged into `into` (a new id
/// over the defaults, a known one over what it has), in the file's order. An empty object is
/// skipped.
pub(super) fn merge_particles(text: &str, into: &mut Vec<ParticleData>) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return false };
    let Some(map) = v.as_object() else { return false };
    // serde_json's map is sorted: keep the file's order by finding each key's place in the text.
    let mut keys: Vec<(usize, &String)> = map.keys().map(|k| (key_position(text, k), k)).collect();
    keys.sort();
    for (_, id) in keys {
        let o = &map[id];
        if !o.is_object() || o.as_object().is_some_and(|m| m.is_empty()) {
            continue;
        }
        match into.iter_mut().find(|p| &p.id == id) {
            Some(p) => p.parse_onto(o),
            None => {
                let mut p = ParticleData { id: id.clone(), ..ParticleData::default() };
                p.parse_onto(o);
                into.push(p);
            }
        }
    }
    true
}

/// Where a top-level key of a JSON object starts in its text (a rough scan, for the order).
fn key_position(text: &str, key: &str) -> usize {
    let quoted = serde_json::to_string(key).unwrap_or_default();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if in_string {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_string = false;
            }
        } else if c == b'"' {
            if depth == 1 && text[i..].starts_with(&quoted) {
                return i;
            }
            in_string = true;
        } else if c == b'{' || c == b'[' {
            depth += 1;
        } else if c == b'}' || c == b']' {
            depth -= 1;
        }
        i += 1;
    }
    usize::MAX
}

/// `saveCustomParticles`' file: every particle by id, what differs from its built-in one.
pub(super) fn particles_json(particles: &[ParticleData], defaults: &[ParticleData]) -> String {
    let mut w = Json::default();
    w.open('{');
    for p in particles {
        w.key(&p.id);
        p.write_saved(defaults.iter().find(|d| d.id == p.id), &mut w);
    }
    w.close('}');
    w.finish()
}

/// The built-in particles (`loadPackParticles`): the original's file, then a resource pack's
/// `assets/particlerain/particles.json` over it.
pub(super) fn pack_particles(pack_file: Option<&str>) -> Vec<ParticleData> {
    let mut out = Vec::new();
    merge_particles(super::particlerain_defaults::PARTICLES_JSON, &mut out);
    if let Some(text) = pack_file
        && !merge_particles(text, &mut out)
    {
        log::warn!("Particle Rain: invalid 'particles.json' in a resource pack");
    }
    out
}

// ---- Gson's pretty printing ------------------------------------------------------------------

/// A JSON writer laid out as Gson's `setPrettyPrinting` (two spaces, `"key": value`, `[]`/`{}`
/// when empty).
#[derive(Default)]
pub(super) struct Json {
    out: String,
    /// Per open container: whether it has an element yet.
    stack: Vec<bool>,
    after_key: bool,
}

impl Json {
    fn indent(&mut self) {
        self.out.push('\n');
        for _ in 0..self.stack.len() {
            self.out.push_str("  ");
        }
    }
    fn element(&mut self) {
        if self.after_key {
            self.after_key = false;
            return;
        }
        if let Some(has) = self.stack.last_mut() {
            if *has {
                self.out.push(',');
            }
            *has = true;
            self.indent();
        }
    }
    fn open(&mut self, c: char) {
        self.element();
        self.out.push(c);
        self.stack.push(false);
    }
    fn close(&mut self, c: char) {
        let had = self.stack.pop().unwrap_or(false);
        if had {
            self.indent();
        }
        self.out.push(c);
    }
    fn key(&mut self, k: &str) {
        self.element();
        self.out.push_str(&serde_json::to_string(k).unwrap_or_default());
        self.out.push_str(": ");
        self.after_key = true;
    }
    fn raw(&mut self, v: &str) {
        self.element();
        self.out.push_str(v);
    }
    fn string(&mut self, v: &str) {
        self.element();
        // Gson escapes `<`, `>`, `&`, `=` and `'` as < etc. (HTML-safe by default).
        let mut s = serde_json::to_string(v).unwrap_or_default();
        for (c, e) in [('<', "\\u003c"), ('>', "\\u003e"), ('&', "\\u0026"), ('=', "\\u003d"), ('\'', "\\u0027")] {
            s = s.replace(c, e);
        }
        self.out.push_str(&s);
    }
    fn strings(&mut self, v: &[String]) {
        self.open('[');
        for s in v {
            self.string(s);
        }
        self.close(']');
    }
    fn int(&mut self, k: &str, v: i32) {
        self.key(k);
        self.raw(&v.to_string());
    }
    fn float(&mut self, k: &str, v: f32) {
        self.key(k);
        self.raw(&json_float(v));
    }
    fn boolean(&mut self, k: &str, v: bool) {
        self.key(k);
        self.raw(&v.to_string());
    }
    fn finish(self) -> String {
        self.out
    }
}

// ---- tags --------------------------------------------------------------------------------------

/// The `c:` convention tags Particle Rain's built-in particles name. Fabric API defines them in
/// Java; Horizon has none, so the port defines these after Fabric API's conventional tags
/// (Apache-2.0), over the game's own tags.
const BLOCK_C_TAGS: &[(&str, &[&str])] = &[
    (
        "c:sandstone_blocks",
        &[
            "minecraft:sandstone",
            "minecraft:chiseled_sandstone",
            "minecraft:cut_sandstone",
            "minecraft:smooth_sandstone",
            "minecraft:red_sandstone",
            "minecraft:chiseled_red_sandstone",
            "minecraft:cut_red_sandstone",
            "minecraft:smooth_red_sandstone",
        ],
    ),
    (
        "c:glass_panes",
        &[
            "minecraft:glass_pane",
            "minecraft:white_stained_glass_pane",
            "minecraft:orange_stained_glass_pane",
            "minecraft:magenta_stained_glass_pane",
            "minecraft:light_blue_stained_glass_pane",
            "minecraft:yellow_stained_glass_pane",
            "minecraft:lime_stained_glass_pane",
            "minecraft:pink_stained_glass_pane",
            "minecraft:gray_stained_glass_pane",
            "minecraft:light_gray_stained_glass_pane",
            "minecraft:cyan_stained_glass_pane",
            "minecraft:purple_stained_glass_pane",
            "minecraft:blue_stained_glass_pane",
            "minecraft:brown_stained_glass_pane",
            "minecraft:green_stained_glass_pane",
            "minecraft:red_stained_glass_pane",
            "minecraft:black_stained_glass_pane",
        ],
    ),
];
const BIOME_C_TAGS: &[(&str, &[&str])] = &[
    ("c:in_overworld", &["#minecraft:is_overworld"]),
    ("c:is_overworld", &["#minecraft:is_overworld"]),
    (
        "c:is_wet",
        &[
            "minecraft:swamp",
            "minecraft:mangrove_swamp",
            "minecraft:jungle",
            "minecraft:sparse_jungle",
            "minecraft:bamboo_jungle",
            "minecraft:beach",
            "minecraft:lush_caves",
            "minecraft:dripstone_caves",
        ],
    ),
    ("c:climate_wet", &["#c:is_wet"]),
    ("c:is_spooky", &["minecraft:dark_forest", "minecraft:pale_garden", "minecraft:deep_dark"]),
    (
        "c:is_windswept",
        &["minecraft:windswept_hills", "minecraft:windswept_gravelly_hills", "minecraft:windswept_forest", "minecraft:windswept_savanna"],
    ),
];

/// Block and biome tags, resolved to their members' ids (nested tags included).
#[derive(Default)]
pub(super) struct Tags {
    blocks: HashMap<String, HashSet<String>>,
    biomes: HashMap<String, HashSet<String>>,
}

/// A tag file's `values`: ids, `#tags`, and `{"id": ..., "required": false}`.
fn tag_values(text: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    v.get("values")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|e| e.as_str().map(str::to_owned).or_else(|| e.get("id").and_then(Value::as_str).map(str::to_owned)))
                .collect()
        })
        .unwrap_or_default()
}

fn resolve(raw: &HashMap<String, Vec<String>>) -> HashMap<String, HashSet<String>> {
    fn walk(name: &str, raw: &HashMap<String, Vec<String>>, seen: &mut HashSet<String>, out: &mut HashSet<String>) {
        if !seen.insert(name.to_owned()) {
            return;
        }
        for v in raw.get(name).into_iter().flatten() {
            match v.strip_prefix('#') {
                Some(t) => walk(&full_id(t), raw, seen, out),
                None => {
                    out.insert(full_id(v));
                }
            }
        }
    }
    raw.keys()
        .map(|k| {
            let mut out = HashSet::new();
            walk(k, raw, &mut HashSet::new(), &mut out);
            (k.clone(), out)
        })
        .collect()
}

impl Tags {
    /// The game's block tags (`block-tags.json`) and biome tags (`data/minecraft/tags/worldgen/biome`)
    /// under `root`, with the convention tags above.
    pub(super) fn load(root: &std::path::Path) -> Tags {
        let mut blocks: HashMap<String, Vec<String>> = HashMap::new();
        if let Ok(bytes) = horizon_core::prepared_files::read(&root.join("block-tags.json"))
            && let Ok(bundle) = serde_json::from_slice::<HashMap<String, String>>(&bytes)
        {
            for (name, text) in bundle {
                blocks.insert(full_id(&name), tag_values(&text));
            }
        }
        let mut biomes: HashMap<String, Vec<String>> = HashMap::new();
        if let Ok(folder) = horizon_world::json_folder::read(&root.join("data/minecraft/tags/worldgen/biome")) {
            for f in &folder.files {
                if let Ok(bytes) = f.bytes() {
                    biomes.insert(format!("minecraft:{}", f.name), tag_values(&String::from_utf8_lossy(bytes)));
                }
            }
        }
        for (name, values) in BLOCK_C_TAGS {
            blocks.entry((*name).to_owned()).or_default().extend(values.iter().map(|v| (*v).to_owned()));
        }
        for (name, values) in BIOME_C_TAGS {
            biomes.entry((*name).to_owned()).or_default().extend(values.iter().map(|v| (*v).to_owned()));
        }
        Tags { blocks: resolve(&blocks), biomes: resolve(&biomes) }
    }
    pub(super) fn block_in(&self, block: &str, tag: &str) -> bool {
        self.blocks.get(tag).is_some_and(|s| s.contains(block))
    }
    pub(super) fn biome_in(&self, biome: &str, tag: &str) -> bool {
        self.biomes.get(tag).is_some_and(|s| s.contains(biome))
    }
    /// A biome's tags, sorted (`/particlerain biometags`).
    pub(super) fn biome_tags(&self, biome: &str) -> Vec<String> {
        let mut v: Vec<String> = self.biomes.iter().filter(|(_, s)| s.contains(biome)).map(|(k, _)| k.clone()).collect();
        v.sort();
        v
    }
}
