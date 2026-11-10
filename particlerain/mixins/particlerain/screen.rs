// Particle Rain's config screens, as the original builds them with the game's widgets
// (`ConfigScreen` in a `HeaderAndFooterLayout`: the title, a `WidgetList` of 310-pixel rows 25
// apart, "Reset <title>" and Done in the footer):
// - the main screen (`MainConfigScreen`): Performance, Sound, Wind and Compatibility Options,
//   each particle's on/off button and Edit (and ❌ for the player's own), and Add new;
// - an options screen per section, its fields by kind (number fields with a label, ON/OFF and
//   value buttons, sliders, and the list of blocks weather ignores);
// - a particle's screen (`ParticleEditScreen`): its fields under "spawning", "motion" and
//   "appearance" (the original's labels name keys it has no text for, so they show as such),
//   only those its style uses, the lists (precipitation, sprites) on screens of their own, the
//   biome and block lists (`Whitelist`) with their entries in place, and the streak's
//   Appearance options;
// - the game's link confirmation for "View common tags online".
// Each change applies at once; leaving a screen saves the files (`ConfigManager.save`,
// `saveCustomParticles`) and reloads the resources when a texture setting changed. Opened by
// `/particlerain` in the world, by Configure... on Options > Mods > Particle Rain and by Mod
// Menu's Configure. Spliced into game.rs as `particlerain_screen`.
//
// Particle Rain by PigCart (https://github.com/PigCart/particle-rain), MIT.

use super::particlerain_core::{self as core, Config, EnumKind, Field, ParticleData, ParticleStyle, TintType, Whitelist};
use super::particlerain_glue as glue;
use super::particlerain_lang::{self as lang, t, tf};
use super::particlerain_weather as weather;
use super::{Game, Screen as GameScreen};
use horizon_client::hud::{HudAtlas, HudBatch, rgba};
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page as OptionsPage;
use horizon_client::text::Text;
use horizon_client::ui::{self, EditBox};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Instant;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after it opened the page.
pub(super) const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

const SHIFT: u32 = 1;
const CONTROL: u32 = 2;
const GREEN: u32 = 0x55_FF55;
const RED: u32 = 0xFF_5555;
/// `EditBox.DEFAULT_TEXT_COLOR`.
const TEXT: u32 = 0xFFE0_E0E0;
const DARK: u32 = 0xFF55_5555;
const BUTTON_WIDTH: f32 = 150.0;
const BIG_BUTTON_WIDTH: f32 = 310.0;
const BUTTON_HEIGHT: f32 = 20.0;
const ROW: f32 = 25.0;
const WIKI_BLOCKS: &str = "https://wiki.fabricmc.net/community:common_tags#block_tags";
const WIKI_BIOMES: &str = "https://wiki.fabricmc.net/community:common_tags#biome_tags";

// ---- the options -------------------------------------------------------------------------------

/// A section of `ConfigData` (`perf`, `sound`, `wind`, `compat`, and `streak` behind the streak's
/// Appearance).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sect {
    Perf,
    Sound,
    Wind,
    Compat,
    Streak,
}

impl Sect {
    fn key(self) -> &'static str {
        match self {
            Sect::Perf => "particlerain.perf",
            Sect::Sound => "particlerain.sound",
            Sect::Wind => "particlerain.wind",
            Sect::Compat => "particlerain.compat",
            Sect::Streak => "particlerain.appearance",
        }
    }
    fn options(self) -> &'static [Opt] {
        match self {
            Sect::Perf => &[Opt::ParticleAmount, Opt::ParticleStormDensity, Opt::ParticleDistance, Opt::SurfaceRange],
            Sect::Sound => &[Opt::RainVolume, Opt::SnowVolume, Opt::WindVolume],
            Sect::Wind => &[Opt::Strength, Opt::GustFrequency, Opt::ModulationSpeed, Opt::YLevelAdjustment, Opt::ChangingWindDirection],
            Sect::Compat => &[
                Opt::RenderDefaultRain,
                Opt::RenderDefaultSnow,
                Opt::DoDefaultSplashing,
                Opt::WaterTint,
                Opt::TintMix,
                Opt::ShaderpackTint,
                Opt::SyncRegistries,
                Opt::CrossBiomeBorder,
                Opt::UseHeightmapTemp,
                Opt::WeatherIgnoreBlocks,
                Opt::DoSpawnHeightLimit,
                Opt::SpawnHeightLimit,
                Opt::RippleRes,
                Opt::RippleUsePackRes,
            ],
            Sect::Streak => &[Opt::StreakOpacity, Opt::StreakSize],
        }
    }
}

/// One option of `ConfigData`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
enum Opt {
    ParticleAmount,
    ParticleStormDensity,
    ParticleDistance,
    SurfaceRange,
    RainVolume,
    SnowVolume,
    WindVolume,
    Strength,
    GustFrequency,
    ModulationSpeed,
    YLevelAdjustment,
    /// The port's addition.
    ChangingWindDirection,
    RenderDefaultRain,
    RenderDefaultSnow,
    DoDefaultSplashing,
    WaterTint,
    TintMix,
    ShaderpackTint,
    SyncRegistries,
    CrossBiomeBorder,
    UseHeightmapTemp,
    WeatherIgnoreBlocks,
    DoSpawnHeightLimit,
    SpawnHeightLimit,
    RippleRes,
    RippleUsePackRes,
    StreakOpacity,
    StreakSize,
}

/// How a value's text reads (`@Format`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fmt {
    Plain,
    Percent,
    PercentOrOff,
    Blocks,
    Auto,
    Ticks,
}

/// What a value is, and how the screen edits it.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Int(Fmt),
    Float(Fmt),
    Bool,
    /// `@Slider`: from 0 to 1 in `step`s.
    Slider(f32, Fmt),
    Str,
    Color,
    Enum(EnumKind),
    /// A list on a screen of its own.
    List,
    /// An object (a `Whitelist`) on a screen of its own.
    Object,
}

/// What changing a value does besides (`@OnChange`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OnChange {
    None,
    ClearParticles,
    ReloadResources,
    Refresh,
}

impl Opt {
    fn name(self) -> &'static str {
        match self {
            Opt::ParticleAmount => "particleAmount",
            Opt::ParticleStormDensity => "particleStormDensity",
            Opt::ParticleDistance => "particleDistance",
            Opt::SurfaceRange => "surfaceRange",
            Opt::RainVolume => "rainVolume",
            Opt::SnowVolume => "snowVolume",
            Opt::WindVolume => "windVolume",
            Opt::Strength => "strength",
            Opt::GustFrequency => "gustFrequency",
            Opt::ModulationSpeed => "modulationSpeed",
            Opt::YLevelAdjustment => "yLevelAdjustment",
            Opt::ChangingWindDirection => "horizonChangingWindDirection",
            Opt::RenderDefaultRain => "renderDefaultRain",
            Opt::RenderDefaultSnow => "renderDefaultSnow",
            Opt::DoDefaultSplashing => "doDefaultSplashing",
            Opt::WaterTint => "waterTint",
            Opt::TintMix => "tintMix",
            Opt::ShaderpackTint => "shaderpackTint",
            Opt::SyncRegistries => "syncRegistries",
            Opt::CrossBiomeBorder => "crossBiomeBorder",
            Opt::UseHeightmapTemp => "useHeightmapTemp",
            Opt::WeatherIgnoreBlocks => "weatherIgnoreBlocks",
            Opt::DoSpawnHeightLimit => "doSpawnHeightLimit",
            Opt::SpawnHeightLimit => "spawnHeightLimit",
            Opt::RippleRes => "rippleRes",
            Opt::RippleUsePackRes => "rippleUsePackRes",
            Opt::StreakOpacity => "opacity",
            Opt::StreakSize => "size",
        }
    }
    fn kind(self) -> Kind {
        match self {
            Opt::ParticleAmount | Opt::RippleRes => Kind::Int(Fmt::Plain),
            Opt::ParticleDistance | Opt::SurfaceRange => Kind::Int(Fmt::Blocks),
            Opt::SpawnHeightLimit => Kind::Int(Fmt::Auto),
            Opt::ParticleStormDensity | Opt::Strength | Opt::GustFrequency | Opt::ModulationSpeed | Opt::StreakSize => Kind::Float(Fmt::Plain),
            Opt::RainVolume | Opt::SnowVolume | Opt::WindVolume => Kind::Slider(0.01, Fmt::PercentOrOff),
            Opt::TintMix | Opt::StreakOpacity => Kind::Slider(0.01, Fmt::Percent),
            Opt::WeatherIgnoreBlocks => Kind::Object,
            _ => Kind::Bool,
        }
    }
    fn on_change(self) -> OnChange {
        match self {
            Opt::ParticleAmount => OnChange::ClearParticles,
            Opt::WaterTint | Opt::RippleRes | Opt::RippleUsePackRes => OnChange::ReloadResources,
            _ => OnChange::None,
        }
    }
    fn get(self, c: &Config) -> Val {
        match self {
            Opt::ParticleAmount => Val::Int(c.perf.particle_amount),
            Opt::ParticleStormDensity => Val::Float(c.perf.particle_storm_density),
            Opt::ParticleDistance => Val::Int(c.perf.particle_distance),
            Opt::SurfaceRange => Val::Int(c.perf.surface_range),
            Opt::RainVolume => Val::Float(c.sound.rain_volume),
            Opt::SnowVolume => Val::Float(c.sound.snow_volume),
            Opt::WindVolume => Val::Float(c.sound.wind_volume),
            Opt::Strength => Val::Float(c.wind.strength),
            Opt::GustFrequency => Val::Float(c.wind.gust_frequency),
            Opt::ModulationSpeed => Val::Float(c.wind.modulation_speed),
            Opt::YLevelAdjustment => Val::Bool(c.wind.y_level_adjustment),
            Opt::ChangingWindDirection => Val::Bool(c.wind.changing_direction),
            Opt::RenderDefaultRain => Val::Bool(c.compat.render_default_rain),
            Opt::RenderDefaultSnow => Val::Bool(c.compat.render_default_snow),
            Opt::DoDefaultSplashing => Val::Bool(c.compat.do_default_splashing),
            Opt::WaterTint => Val::Bool(c.compat.water_tint),
            Opt::TintMix => Val::Float(c.compat.tint_mix),
            Opt::ShaderpackTint => Val::Bool(c.compat.shaderpack_tint),
            Opt::SyncRegistries => Val::Bool(c.compat.sync_registries),
            Opt::CrossBiomeBorder => Val::Bool(c.compat.cross_biome_border),
            Opt::UseHeightmapTemp => Val::Bool(c.compat.use_heightmap_temp),
            Opt::WeatherIgnoreBlocks => Val::None,
            Opt::DoSpawnHeightLimit => Val::Bool(c.compat.do_spawn_height_limit),
            Opt::SpawnHeightLimit => Val::Int(c.compat.spawn_height_limit),
            Opt::RippleRes => Val::Int(c.compat.ripple_res),
            Opt::RippleUsePackRes => Val::Bool(c.compat.ripple_use_pack_res),
            Opt::StreakOpacity => Val::Float(c.streak.opacity),
            Opt::StreakSize => Val::Float(c.streak.size),
        }
    }
    fn set(self, c: &mut Config, v: &Val) {
        match (self, v) {
            (Opt::ParticleAmount, Val::Int(i)) => c.perf.particle_amount = *i,
            (Opt::ParticleStormDensity, Val::Float(f)) => c.perf.particle_storm_density = *f,
            (Opt::ParticleDistance, Val::Int(i)) => c.perf.particle_distance = *i,
            (Opt::SurfaceRange, Val::Int(i)) => c.perf.surface_range = *i,
            (Opt::RainVolume, Val::Float(f)) => c.sound.rain_volume = *f,
            (Opt::SnowVolume, Val::Float(f)) => c.sound.snow_volume = *f,
            (Opt::WindVolume, Val::Float(f)) => c.sound.wind_volume = *f,
            (Opt::Strength, Val::Float(f)) => c.wind.strength = *f,
            (Opt::GustFrequency, Val::Float(f)) => c.wind.gust_frequency = *f,
            (Opt::ModulationSpeed, Val::Float(f)) => c.wind.modulation_speed = *f,
            (Opt::YLevelAdjustment, Val::Bool(b)) => c.wind.y_level_adjustment = *b,
            (Opt::ChangingWindDirection, Val::Bool(b)) => c.wind.changing_direction = *b,
            (Opt::RenderDefaultRain, Val::Bool(b)) => c.compat.render_default_rain = *b,
            (Opt::RenderDefaultSnow, Val::Bool(b)) => c.compat.render_default_snow = *b,
            (Opt::DoDefaultSplashing, Val::Bool(b)) => c.compat.do_default_splashing = *b,
            (Opt::WaterTint, Val::Bool(b)) => c.compat.water_tint = *b,
            (Opt::TintMix, Val::Float(f)) => c.compat.tint_mix = *f,
            (Opt::ShaderpackTint, Val::Bool(b)) => c.compat.shaderpack_tint = *b,
            (Opt::SyncRegistries, Val::Bool(b)) => c.compat.sync_registries = *b,
            (Opt::CrossBiomeBorder, Val::Bool(b)) => c.compat.cross_biome_border = *b,
            (Opt::UseHeightmapTemp, Val::Bool(b)) => c.compat.use_heightmap_temp = *b,
            (Opt::DoSpawnHeightLimit, Val::Bool(b)) => c.compat.do_spawn_height_limit = *b,
            (Opt::SpawnHeightLimit, Val::Int(i)) => c.compat.spawn_height_limit = *i,
            (Opt::RippleRes, Val::Int(i)) => c.compat.ripple_res = *i,
            (Opt::RippleUsePackRes, Val::Bool(b)) => c.compat.ripple_use_pack_res = *b,
            (Opt::StreakOpacity, Val::Float(f)) => c.streak.opacity = *f,
            (Opt::StreakSize, Val::Float(f)) => c.streak.size = *f,
            _ => {}
        }
    }
}

/// A value as the screen reads and writes it.
#[derive(Clone, PartialEq, Debug)]
enum Val {
    None,
    Int(i32),
    Float(f32),
    Bool(bool),
    Str(String),
    Color(u32),
    Enum(u8),
}

fn field_kind(f: Field) -> Kind {
    match f {
        Field::ParticleStyle => Kind::Enum(EnumKind::ParticleStyle),
        Field::RegisteredParticleId | Field::BlockId | Field::Id => Kind::Str,
        Field::Enabled | Field::NeedsSkyAccess | Field::AnimateSprites => Kind::Bool,
        Field::Density => Kind::Slider(0.001, Fmt::Percent),
        Field::Weather => Kind::Enum(EnumKind::Weather),
        Field::Precipitation | Field::SpriteLocations => Kind::List,
        Field::BiomeList | Field::BlockList => Kind::Object,
        Field::SpawnPos => Kind::Enum(EnumKind::SpawnPos),
        Field::WindStrength | Field::StormWindStrength | Field::Gravity | Field::RotationAmount | Field::Bounciness => Kind::Float(Fmt::Plain),
        Field::Size => Kind::Float(Fmt::Blocks),
        Field::Lifetime => Kind::Int(Fmt::Ticks),
        Field::Opacity | Field::DistanceScaling => Kind::Slider(0.01, Fmt::Percent),
        Field::FadeType => Kind::Enum(EnumKind::FadeType),
        Field::RenderType => Kind::Enum(EnumKind::RenderType),
        Field::TintType => Kind::Enum(EnumKind::TintType),
        Field::CustomTint => Kind::Color,
        Field::RotationType => Kind::Enum(EnumKind::RotationType),
    }
}

fn field_on_change(f: Field) -> OnChange {
    match f {
        Field::ParticleStyle | Field::TintType | Field::RotationType => OnChange::Refresh,
        _ => OnChange::None,
    }
}

/// `@Label` before a field (the original's keys, which have no text).
fn field_label(f: Field) -> Option<&'static str> {
    match f {
        Field::Enabled => Some("spawning"),
        Field::WindStrength => Some("motion"),
        Field::Opacity => Some("appearance"),
        _ => None,
    }
}

/// `@OnlyVisibleIf`.
fn field_visible(f: Field, d: &ParticleData, pack: bool) -> bool {
    let registered = d.particle_style == ParticleStyle::Registered;
    match f {
        Field::RegisteredParticleId => registered,
        Field::BlockId => d.particle_style == ParticleStyle::BlockModel,
        Field::Id => !pack,
        Field::Gravity
        | Field::RotationAmount
        | Field::Bounciness
        | Field::Lifetime
        | Field::Opacity
        | Field::FadeType
        | Field::Size
        | Field::DistanceScaling
        | Field::RenderType
        | Field::AnimateSprites
        | Field::SpriteLocations
        | Field::TintType => !registered,
        Field::CustomTint => d.tint_type == TintType::Custom,
        Field::RotationType => d.particle_style == ParticleStyle::Custom,
        _ => true,
    }
}

fn field_get(f: Field, d: &ParticleData) -> Val {
    match f {
        Field::ParticleStyle => Val::Enum(d.particle_style.index()),
        Field::RegisteredParticleId => Val::Str(d.registered_particle_id.clone()),
        Field::BlockId => Val::Str(d.block_id.clone()),
        Field::Id => Val::Str(d.id.clone()),
        Field::Enabled => Val::Bool(d.enabled),
        Field::Density => Val::Float(d.density),
        Field::Weather => Val::Enum(d.weather.index()),
        Field::NeedsSkyAccess => Val::Bool(d.needs_sky_access),
        Field::SpawnPos => Val::Enum(d.spawn_pos.index()),
        Field::WindStrength => Val::Float(d.wind_strength),
        Field::StormWindStrength => Val::Float(d.storm_wind_strength),
        Field::Gravity => Val::Float(d.gravity),
        Field::RotationAmount => Val::Float(d.rotation_amount),
        Field::Bounciness => Val::Float(d.bounciness),
        Field::Lifetime => Val::Int(d.lifetime),
        Field::Opacity => Val::Float(d.opacity),
        Field::FadeType => Val::Enum(d.fade_type.index()),
        Field::Size => Val::Float(d.size),
        Field::DistanceScaling => Val::Float(d.distance_scaling),
        Field::RenderType => Val::Enum(d.render_type.index()),
        Field::AnimateSprites => Val::Bool(d.animate_sprites),
        Field::TintType => Val::Enum(d.tint_type.index()),
        Field::CustomTint => Val::Color(d.custom_tint),
        Field::RotationType => Val::Enum(d.rotation_type.index()),
        Field::Precipitation | Field::SpriteLocations | Field::BiomeList | Field::BlockList => Val::None,
    }
}

fn field_set(f: Field, d: &mut ParticleData, v: &Val) {
    use super::particlerain_core::{FadeType, RenderType, RotationType, SpawnPos, Weather};
    match (f, v) {
        (Field::ParticleStyle, Val::Enum(i)) => d.particle_style = ParticleStyle::from_index(*i),
        (Field::RegisteredParticleId, Val::Str(s)) => d.registered_particle_id = s.clone(),
        (Field::BlockId, Val::Str(s)) => d.block_id = s.clone(),
        (Field::Id, Val::Str(s)) => d.id = s.clone(),
        (Field::Enabled, Val::Bool(b)) => d.enabled = *b,
        (Field::Density, Val::Float(x)) => d.density = *x,
        (Field::Weather, Val::Enum(i)) => d.weather = Weather::from_index(*i),
        (Field::NeedsSkyAccess, Val::Bool(b)) => d.needs_sky_access = *b,
        (Field::SpawnPos, Val::Enum(i)) => d.spawn_pos = SpawnPos::from_index(*i),
        (Field::WindStrength, Val::Float(x)) => d.wind_strength = *x,
        (Field::StormWindStrength, Val::Float(x)) => d.storm_wind_strength = *x,
        (Field::Gravity, Val::Float(x)) => d.gravity = *x,
        (Field::RotationAmount, Val::Float(x)) => d.rotation_amount = *x,
        (Field::Bounciness, Val::Float(x)) => d.bounciness = *x,
        (Field::Lifetime, Val::Int(i)) => d.lifetime = *i,
        (Field::Opacity, Val::Float(x)) => d.opacity = *x,
        (Field::FadeType, Val::Enum(i)) => d.fade_type = FadeType::from_index(*i),
        (Field::Size, Val::Float(x)) => d.size = *x,
        (Field::DistanceScaling, Val::Float(x)) => d.distance_scaling = *x,
        (Field::RenderType, Val::Enum(i)) => d.render_type = RenderType::from_index(*i),
        (Field::AnimateSprites, Val::Bool(b)) => d.animate_sprites = *b,
        (Field::TintType, Val::Enum(i)) => d.tint_type = TintType::from_index(*i),
        (Field::CustomTint, Val::Color(c)) => d.custom_tint = *c,
        (Field::RotationType, Val::Enum(i)) => d.rotation_type = RotationType::from_index(*i),
        _ => {}
    }
}

// ---- texts --------------------------------------------------------------------------------------

/// `NumberFormat.getPercentInstance().format`: whole percent, half to even, the locale's sign.
fn percent(v: f32) -> String {
    let x = f64::from(v) * 100.0;
    let r = x.round();
    let r = if (x - x.trunc()).abs() == 0.5 && r % 2.0 != 0.0 { r - x.signum() } else { r };
    let n = r as i64;
    let digits = n.abs().to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(if lang::russian() { '\u{a0}' } else { ',' });
        }
        grouped.push(c);
    }
    let sign = if n < 0 { "-" } else { "" };
    if lang::russian() { format!("{sign}{grouped}\u{a0}%") } else { format!("{sign}{grouped}%") }
}

/// A `@Format`'s text for a value's text (an input's, or a slider's number).
fn formatted(fmt: Fmt, text: &str) -> Text {
    match fmt {
        Fmt::Plain => Text::literal(text.to_owned()),
        Fmt::Percent => Text::literal(text.parse::<f32>().map(percent).unwrap_or_else(|_| text.to_owned())),
        Fmt::PercentOrOff => match text.parse::<f32>() {
            Ok(v) if v == 0.0 => Text::translate("options.off", &[]).with_color(RED),
            Ok(v) => Text::literal(percent(v)),
            Err(_) => Text::literal(text.to_owned()),
        },
        Fmt::Blocks => Text::literal(tf("particlerain.distanceInblocks", &[text])),
        Fmt::Auto => match text.trim().parse::<i32>() {
            Ok(0) => Text::literal(t("particlerain.auto")),
            _ => Text::literal(text.to_owned()),
        },
        Fmt::Ticks => match text.trim().parse::<i32>() {
            Ok(i) => Text::literal(tf("particlerain.timeInTicks", &[text, &core::java_float(i as f32 / 20.0)])),
            Err(_) => Text::literal(text.to_owned()),
        },
    }
}

/// `DecimalFormat` with at most three decimals (Locale.ROOT), as a float field starts.
fn decimal(v: f32) -> String {
    let s = format!("{:.3}", f64::from(v));
    let s = s.trim_end_matches('0').trim_end_matches('.').to_owned();
    let (sign, s) = match s.strip_prefix('-') {
        Some(r) => ("-", r.to_owned()),
        None => ("", s),
    };
    let (int, frac) = s.split_once('.').map_or((s.clone(), String::new()), |(a, b)| (a.to_owned(), format!(".{b}")));
    let mut grouped = String::new();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let out = format!("{sign}{grouped}{frac}");
    if out == "-0" { "-0".into() } else { out }
}

/// `Float.valueOf`.
fn parse_float(s: &str) -> Option<f32> {
    let s = s.trim();
    let s = s.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(s);
    if s.is_empty() || s.contains(',') {
        return None;
    }
    s.parse::<f32>().ok()
}

/// `Integer.valueOf`.
fn parse_int(s: &str) -> Option<i32> {
    if s.is_empty() || s.chars().skip(usize::from(s.starts_with(['-', '+']))).any(|c| !c.is_ascii_digit()) {
        return None;
    }
    s.parse::<i32>().ok()
}

fn on_off(b: bool) -> Text {
    if b { Text::translate("options.on", &[]).with_color(GREEN) } else { Text::translate("options.off", &[]).with_color(RED) }
}

fn enum_text(kind: EnumKind, i: u8) -> Text {
    let value = kind.values().get(usize::from(i)).copied().unwrap_or("");
    Text::literal(t(&format!("particlerain.{}.{}", kind.class(), value)))
}

/// `CommonComponents.optionNameValue`.
fn name_value(name: &str, value: Text) -> Text {
    Text::option_name_value(Text::literal(name.to_owned()), value)
}

// ---- the screens' stack ----------------------------------------------------------------------

/// Which particle list a list screen shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ListOf {
    Precipitation,
    Sprites,
}

/// Which `Whitelist` a list screen shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ListObj {
    Biomes,
    Blocks,
    Ignored,
}

#[derive(Clone, Debug)]
enum Kind2 {
    Main,
    Section(Sect),
    /// A particle's screen: what it edits (the particle itself for a built-in one, written back
    /// at once; the player's own taken out of the list until the screen goes), whether it is
    /// built in, and its title.
    Particle { data: Box<ParticleData>, pack: bool, title: String },
    List(ListOf),
    Object(ListObj),
    Link(String),
}

/// An input field's state.
struct Input {
    edit: EditBox,
    valid: bool,
}

/// What a screen keeps.
struct Page {
    kind: Kind2,
    scroll: f32,
    inputs: HashMap<String, Input>,
    focus: Option<String>,
}

impl Page {
    fn new(kind: Kind2) -> Page {
        Page { kind, scroll: 0.0, inputs: HashMap::new(), focus: None }
    }
}

/// Where the screens are.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// In the world, a screen of the mod's own (no parent: closing goes back to the game).
    World,
    /// Over the mod's page in Options > Mods (row `row`); `modmenu`: Mod Menu opened it.
    Page { row: usize, modmenu: bool },
}

pub(super) enum Request {
    Click,
    Paste,
    Copy(String),
    OpenUrl(String),
    ClearParticles,
    Reload,
}

struct State {
    mode: Mode,
    stack: Vec<Page>,
    /// The slider held by the pointer (its key).
    dragging: Option<String>,
    /// The scrollbar held: the pointer's last y.
    scrolling: Option<f32>,
    taken: u8,
    /// The focus came from the keyboard (`InputType.isKeyboard`): only then does it show.
    keyboard: bool,
    close: bool,
    requests: Vec<Request>,
    opened: Instant,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().and_then(|mut s| s.as_mut().map(f)))
}

fn new_state(mode: Mode) -> State {
    State {
        mode,
        stack: vec![Page::new(Kind2::Main)],
        dragging: None,
        scrolling: None,
        taken: 0,
        keyboard: false,
        close: false,
        requests: Vec::new(),
        opened: Instant::now(),
    }
}

/// The mod's row in Options > Mods, while the options show it.
fn options_row(game: &Game<'_>) -> Option<usize> {
    if game.screen != GameScreen::Options {
        return None;
    }
    match game.ui.options.page() {
        OptionsPage::NativeMod(i) if game.settings.mods.installed.get(i).is_some_and(|m| m.id == glue::ID) => Some(i),
        _ => None,
    }
}

/// `/particlerain`: the screens in the world.
pub(super) fn open_in_world(game: &mut Game<'_>) {
    if game.native_mod_screen_open(glue::ID) {
        STATE.with(|s| *s.borrow_mut() = Some(new_state(Mode::World)));
    }
}

/// Configure... on the mod's page, or Mod Menu's Configure.
pub(super) fn open_over_options(game: &Game<'_>, modmenu: bool) {
    let Some(row) = options_row(game) else { return };
    STATE.with(|s| *s.borrow_mut() = Some(new_state(Mode::Page { row, modmenu })));
}

/// The screens are up over the mod's page: the options stay hidden.
pub(super) fn covers(game: &Game<'_>) -> bool {
    with(|s| matches!(s.mode, Mode::Page { .. })).unwrap_or(false) && options_row(game).is_some()
}

fn shown(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    match with(|s| s.mode) {
        Some(Mode::World) => screen.id == "mod" && game.native_mod_screen() == Some(glue::ID),
        Some(Mode::Page { row, .. }) => screen.id == "options" && options_row(game) == Some(row),
        None => false,
    }
}

// ---- the model ----------------------------------------------------------------------------------

/// The particle the screen at `i` (or the nearest one under it) edits.
fn particle_page(stack: &mut [Page], i: usize) -> Option<(&mut ParticleData, bool)> {
    stack[..=i].iter_mut().rev().find_map(|p| match &mut p.kind {
        Kind2::Particle { data, pack, .. } => Some((&mut **data, *pack)),
        _ => None,
    })
}

/// A built-in particle's change, into the live list at once (Java edits the very object).
fn write_back(data: &ParticleData, pack: bool) {
    if pack {
        glue::with(|st| {
            if let Some(p) = st.particles.iter_mut().find(|p| p.id == data.id) {
                *p = data.clone();
            }
        });
    }
}

/// The `Whitelist` an object screen edits.
fn with_object<R>(stack: &mut [Page], top: usize, obj: ListObj, f: impl FnOnce(&mut Whitelist) -> R) -> Option<R> {
    if obj == ListObj::Ignored {
        return Some(glue::with(|st| f(&mut st.config.compat.weather_ignore_blocks)));
    }
    let (data, pack) = particle_page(stack, top)?;
    let r = f(if obj == ListObj::Biomes { &mut data.biome_list } else { &mut data.block_list });
    write_back(data, pack);
    Some(r)
}

/// A row's widget.
#[derive(Clone, Debug)]
enum Cell {
    /// `LabelWidget`: text aligned in the cell (0.5 centred, 1 right).
    Label(Text, f32),
    Button(Text, Act),
    /// A `CycleButton`: its message, and what a click (or Shift-click) does.
    Cycle(Text, Act),
    /// A slider: its key, message and position.
    Slider(String, Text, f32),
    /// An input field: its key, start text and `@Format`.
    Input(String, String, Fmt),
}

/// What a button does.
#[derive(Clone, Debug, PartialEq)]
enum Act {
    Open(Kind2Key),
    Toggle(usize),
    Edit(usize),
    Remove(usize),
    AddParticle,
    Cycle(String),
    ListRemove(usize),
    ListAdd,
    Link(String),
}

/// A screen to open, as a button names it.
#[derive(Clone, Debug, PartialEq)]
enum Kind2Key {
    Section(Sect),
    List(ListOf),
    Object(ListObj),
}

struct Placed {
    cell: Cell,
    /// Offset from the row's left (`width / 2 - 155`) and width.
    x: f32,
    w: f32,
    tooltip: Option<String>,
}

type Row = Vec<Placed>;

fn placed(cell: Cell, x: f32, w: f32, tooltip: Option<String>) -> Placed {
    Placed { cell, x, w, tooltip }
}

/// `Component.translatableWithFallback(name + ".description", "")`.
fn description(name: &str) -> Option<String> {
    lang::get(&format!("{name}.description")).map(str::to_owned).filter(|s| !s.is_empty())
}

/// The rows of an option or field (`getRowWidgets`).
fn value_rows(rows: &mut Vec<Row>, key: String, name: &str, kind: Kind, value: Val) {
    let tip = description(name);
    let label = t(name);
    let row = match (kind, value) {
        (Kind::Slider(_, fmt), Val::Float(v)) => {
            let text = name_value_plain(&label, formatted(fmt, &java_text(v)));
            vec![placed(Cell::Slider(key, text, v), 0.0, BIG_BUTTON_WIDTH, tip)]
        }
        (Kind::Bool, Val::Bool(b)) => vec![placed(Cell::Cycle(name_value(&label, on_off(b)), Act::Cycle(key)), 0.0, BIG_BUTTON_WIDTH, tip)],
        (Kind::Float(fmt), Val::Float(v)) => vec![
            placed(Cell::Label(Text::literal(format!("{label}:")), 1.0), 0.0, BUTTON_WIDTH, tip.clone()),
            placed(Cell::Input(key, decimal(v), fmt), BUTTON_WIDTH + 8.0, BUTTON_WIDTH, tip),
        ],
        (Kind::Int(fmt), Val::Int(i)) => vec![
            placed(Cell::Label(Text::literal(format!("{label}:")), 1.0), 0.0, BUTTON_WIDTH, tip.clone()),
            placed(Cell::Input(key, i.to_string(), fmt), BUTTON_WIDTH + 8.0, BUTTON_WIDTH, tip),
        ],
        (Kind::Str, Val::Str(s)) => vec![
            placed(Cell::Label(Text::literal(format!("{label}:")), 1.0), 0.0, BUTTON_WIDTH, tip.clone()),
            placed(Cell::Input(key, s, Fmt::Plain), BUTTON_WIDTH + 8.0, BUTTON_WIDTH, tip),
        ],
        (Kind::Color, Val::Color(c)) => vec![
            placed(Cell::Label(Text::literal(format!("{label}:")), 1.0), 0.0, BUTTON_WIDTH, tip.clone()),
            placed(Cell::Input(key, core::color_string(c), Fmt::Plain), BUTTON_WIDTH + 8.0, BUTTON_WIDTH, tip),
        ],
        (Kind::Enum(k), Val::Enum(i)) => vec![placed(Cell::Cycle(name_value(&label, enum_text(k, i)), Act::Cycle(key)), 0.0, BIG_BUTTON_WIDTH, tip)],
        _ => return,
    };
    rows.push(row);
}

/// A slider's message: "Name: value".
fn name_value_plain(name: &str, value: Text) -> Text {
    Text::literal(format!("{name}: ")).append(value)
}

/// A slider's value as its formatter reads it (`Float.toString`).
fn java_text(v: f32) -> String {
    core::java_float(v)
}

/// The rows of a list's entries and its Add new (`addListOptions`).
fn list_rows(rows: &mut Vec<Row>, entries: &[Val], kind: Kind) {
    if entries.is_empty() {
        rows.push(vec![placed(Cell::Label(Text::translate("mco.configure.world.slot.empty", &[]), 0.5), 0.0, BIG_BUTTON_WIDTH, None)]);
    }
    for (i, e) in entries.iter().enumerate() {
        let w = BIG_BUTTON_WIDTH - BUTTON_HEIGHT - 2.0;
        let cell = match (kind, e) {
            (Kind::Enum(k), Val::Enum(v)) => Cell::Cycle(enum_text(k, *v), Act::Cycle(format!("item{i}"))),
            (_, Val::Str(s)) => Cell::Input(format!("item{i}"), s.clone(), Fmt::Plain),
            _ => continue,
        };
        rows.push(vec![
            placed(cell, 0.0, w, None),
            placed(Cell::Button(Text::colored("❌", RED), Act::ListRemove(i)), BIG_BUTTON_WIDTH - BUTTON_HEIGHT, BUTTON_HEIGHT, None),
        ]);
    }
    rows.push(vec![placed(Cell::Button(Text::literal(t("particlerain.addNew")), Act::ListAdd), 0.0, BIG_BUTTON_WIDTH, None)]);
}

/// The top screen's title and rows.
fn build(s: &mut State) -> (String, Vec<Row>) {
    let top = s.stack.len() - 1;
    let mut rows: Vec<Row> = Vec::new();
    let kind = s.stack[top].kind.clone();
    let title = match &kind {
        Kind2::Main => {
            for sect in [Sect::Perf, Sect::Sound, Sect::Wind, Sect::Compat] {
                rows.push(vec![placed(Cell::Button(Text::literal(t(sect.key())), Act::Open(Kind2Key::Section(sect))), 0.0, BIG_BUTTON_WIDTH, description(sect.key()))]);
            }
            let (particles, pack): (Vec<(String, bool)>, Vec<String>) =
                glue::with(|st| (st.particles.iter().map(|p| (p.id.clone(), p.enabled)).collect(), st.pack.iter().map(|p| p.id.clone()).collect()));
            for (i, (id, enabled)) in particles.iter().enumerate() {
                let name = t(&format!("particlerain.{id}"));
                let mut row = vec![placed(Cell::Cycle(name_value(&name, on_off(*enabled)), Act::Toggle(i)), 0.0, BUTTON_WIDTH, None)];
                let edit = Text::translate("selectWorld.edit", &[]);
                if pack.contains(id) {
                    row.push(placed(Cell::Button(edit, Act::Edit(i)), BUTTON_WIDTH + 10.0, BUTTON_WIDTH, None));
                } else {
                    row.push(placed(Cell::Button(edit, Act::Edit(i)), BUTTON_WIDTH + 10.0, BUTTON_WIDTH - BUTTON_HEIGHT, None));
                    row.push(placed(Cell::Button(Text::colored("❌", RED), Act::Remove(i)), BIG_BUTTON_WIDTH - BUTTON_HEIGHT, BUTTON_HEIGHT, None));
                }
                rows.push(row);
            }
            rows.push(vec![placed(Cell::Button(Text::literal(t("particlerain.addNew")), Act::AddParticle), 0.0, BIG_BUTTON_WIDTH, None)]);
            glue::title()
        }
        Kind2::Section(sect) => {
            let cfg = glue::with(|st| st.config.clone());
            for &o in sect.options() {
                let name = format!("particlerain.{}", o.name());
                if o.kind() == Kind::Object {
                    rows.push(vec![placed(Cell::Button(Text::literal(t(&name)), Act::Open(Kind2Key::Object(ListObj::Ignored))), 0.0, BIG_BUTTON_WIDTH, description(&name))]);
                    continue;
                }
                value_rows(&mut rows, format!("{o:?}"), &name, o.kind(), o.get(&cfg));
            }
            t(sect.key())
        }
        Kind2::Particle { data, pack, title } => {
            for f in Field::ALL {
                if !field_visible(f, data, *pack) {
                    continue;
                }
                if let Some(label) = field_label(f) {
                    rows.push(vec![placed(Cell::Label(Text::literal(t(label)), 0.5), 0.0, BIG_BUTTON_WIDTH, None)]);
                }
                let name = format!("particlerain.{}", f.name());
                match field_kind(f) {
                    Kind::List => {
                        let of = if f == Field::Precipitation { ListOf::Precipitation } else { ListOf::Sprites };
                        rows.push(vec![placed(Cell::Button(Text::literal(t(&name)), Act::Open(Kind2Key::List(of))), 0.0, BIG_BUTTON_WIDTH, None)]);
                    }
                    Kind::Object => {
                        let obj = if f == Field::BiomeList { ListObj::Biomes } else { ListObj::Blocks };
                        rows.push(vec![placed(Cell::Button(Text::literal(t(&name)), Act::Open(Kind2Key::Object(obj))), 0.0, BIG_BUTTON_WIDTH, description(&name))]);
                    }
                    k => value_rows(&mut rows, format!("{f:?}"), &name, k, field_get(f, data)),
                }
            }
            // The streak's own options (`legacyParticleIds`).
            if data.particle_style == ParticleStyle::Registered && data.id == "streak" {
                rows.push(vec![placed(Cell::Button(Text::literal(t("particlerain.appearance")), Act::Open(Kind2Key::Section(Sect::Streak))), 0.0, BIG_BUTTON_WIDTH, None)]);
            }
            title.clone()
        }
        Kind2::List(of) => {
            let (entries, kind, name) = match particle_page(&mut s.stack, top) {
                Some((d, _)) => match of {
                    ListOf::Precipitation => (d.precipitation.iter().map(|p| Val::Enum(p.index())).collect::<Vec<_>>(), Kind::Enum(EnumKind::Precipitation), "particlerain.precipitation"),
                    ListOf::Sprites => (d.sprite_locations.iter().map(|s| Val::Str(s.clone())).collect(), Kind::Str, "particlerain.spriteLocations"),
                },
                None => (Vec::new(), Kind::Str, ""),
            };
            list_rows(&mut rows, &entries, kind);
            t(name)
        }
        Kind2::Object(obj) => {
            let wl = with_object(&mut s.stack, top, *obj, |w| w.clone()).unwrap_or_else(|| Whitelist::new(true, &[]));
            let url = if *obj == ListObj::Biomes { WIKI_BIOMES } else { WIKI_BLOCKS };
            rows.push(vec![placed(Cell::Button(Text::literal(t("particlerain.wikiLink")), Act::Link(url.to_owned())), 0.0, BIG_BUTTON_WIDTH, None)]);
            let entries: Vec<Val> = wl.entries.iter().map(|e| Val::Str(e.clone())).collect();
            list_rows(&mut rows, &entries, Kind::Str);
            let mode = Text::literal(t(if wl.is_whitelist { "particlerain.whitelist" } else { "particlerain.blacklist" }));
            rows.push(vec![placed(Cell::Cycle(name_value(&t("particlerain.isWhitelist"), mode), Act::Cycle("isWhitelist".into())), 0.0, BIG_BUTTON_WIDTH, None)]);
            t(match obj {
                ListObj::Biomes => "particlerain.biomeList",
                ListObj::Blocks => "particlerain.blockList",
                ListObj::Ignored => "particlerain.weatherIgnoreBlocks",
            })
        }
        Kind2::Link(_) => String::new(),
    };
    (title, rows)
}

/// A value of the top screen by its key.
fn value_of(s: &mut State, key: &str) -> Val {
    let top = s.stack.len() - 1;
    match s.stack[top].kind.clone() {
        Kind2::Section(sect) => {
            let cfg = glue::with(|st| st.config.clone());
            sect.options().iter().find(|o| format!("{o:?}") == key).map_or(Val::None, |o| o.get(&cfg))
        }
        Kind2::Particle { data, .. } => Field::ALL.iter().find(|f| format!("{f:?}") == key).map_or(Val::None, |f| field_get(*f, &data)),
        Kind2::List(of) => {
            let Some(i) = key.strip_prefix("item").and_then(|i| i.parse::<usize>().ok()) else { return Val::None };
            match (particle_page(&mut s.stack, top), of) {
                (Some((d, _)), ListOf::Precipitation) => d.precipitation.get(i).map_or(Val::None, |p| Val::Enum(p.index())),
                (Some((d, _)), ListOf::Sprites) => d.sprite_locations.get(i).map_or(Val::None, |p| Val::Str(p.clone())),
                _ => Val::None,
            }
        }
        Kind2::Object(obj) => {
            let Some(i) = key.strip_prefix("item").and_then(|i| i.parse::<usize>().ok()) else { return Val::None };
            with_object(&mut s.stack, top, obj, |w| w.entries.get(i).cloned()).flatten().map_or(Val::None, Val::Str)
        }
        _ => Val::None,
    }
}

/// `setConfigField`: the value, then its `@OnChange`.
fn set_value(s: &mut State, key: &str, v: Val) {
    let top = s.stack.len() - 1;
    let mut change = OnChange::None;
    match s.stack[top].kind.clone() {
        Kind2::Section(sect) => {
            if let Some(o) = sect.options().iter().find(|o| format!("{o:?}") == key) {
                let changed = glue::with(|st| {
                    let old = o.get(&st.config);
                    o.set(&mut st.config, &v);
                    old != o.get(&st.config)
                });
                if changed {
                    change = o.on_change();
                }
            }
        }
        Kind2::Particle { .. } => {
            if let Some(f) = Field::ALL.iter().find(|f| format!("{f:?}") == key).copied()
                && let Kind2::Particle { data, pack, .. } = &mut s.stack[top].kind
            {
                let old = field_get(f, data);
                field_set(f, data, &v);
                write_back(data, *pack);
                if old != field_get(f, data) {
                    change = field_on_change(f);
                }
            }
        }
        Kind2::List(of) => {
            let Some(i) = key.strip_prefix("item").and_then(|i| i.parse::<usize>().ok()) else { return };
            if let Some((d, pack)) = particle_page(&mut s.stack, top) {
                match (of, &v) {
                    (ListOf::Precipitation, Val::Enum(e)) => {
                        if let Some(p) = d.precipitation.get_mut(i) {
                            *p = core::Precipitation::from_index(*e);
                        }
                    }
                    (ListOf::Sprites, Val::Str(x)) => {
                        if let Some(p) = d.sprite_locations.get_mut(i) {
                            *p = x.clone();
                        }
                    }
                    _ => {}
                }
                write_back(d, pack);
            }
        }
        Kind2::Object(obj) => {
            with_object(&mut s.stack, top, obj, |w| match (key, &v) {
                ("isWhitelist", Val::Bool(b)) => w.is_whitelist = *b,
                (k, Val::Str(x)) => {
                    if let Some(e) = k.strip_prefix("item").and_then(|i| i.parse::<usize>().ok()).and_then(|i| w.entries.get_mut(i)) {
                        *e = x.clone();
                    }
                }
                _ => {}
            });
        }
        _ => {}
    }
    match change {
        OnChange::None => {}
        OnChange::ClearParticles => s.requests.push(Request::ClearParticles),
        OnChange::ReloadResources => glue::with(|st| st.reload_resources = true),
        OnChange::Refresh => refresh(s),
    }
}

/// `refresh`: the screen built again (fields and focus anew), its scroll kept.
fn refresh(s: &mut State) {
    SLIDERS.with(|m| m.borrow_mut().clear());
    if let Some(p) = s.stack.last_mut() {
        p.inputs.clear();
        p.focus = None;
    }
}

/// `removed`: a particle's screen puts the player's own particle back (by its id); the others
/// save the files and reload the resources when asked.
fn leave(s: &mut State, page: &Page) {
    match &page.kind {
        Kind2::Particle { data, pack: false, .. } => {
            let data = (**data).clone();
            glue::with(|st| match st.particles.iter_mut().find(|p| p.id == data.id) {
                Some(p) => *p = data,
                None => st.particles.push(data),
            });
        }
        Kind2::Particle { .. } | Kind2::Link(_) => {}
        _ => {
            glue::save();
            if glue::with(|st| std::mem::take(&mut st.reload_resources)) {
                s.requests.push(Request::Reload);
            }
        }
    }
}

/// Another screen over this one (`setScreen`).
fn push(s: &mut State, kind: Kind2) {
    let Some(page) = s.stack.pop() else { return };
    leave(s, &page);
    // The screen comes back built anew (`getFreshScreen`), its scroll kept.
    let mut page = page;
    page.inputs.clear();
    page.focus = None;
    s.stack.push(page);
    s.stack.push(Page::new(kind));
    s.dragging = None;
    SLIDERS.with(|m| m.borrow_mut().clear());
}

/// `onClose`: back to the screen under (closing the last one).
fn pop(s: &mut State) {
    let Some(page) = s.stack.pop() else { return };
    leave(s, &page);
    s.dragging = None;
    SLIDERS.with(|m| m.borrow_mut().clear());
    if let Some(under) = s.stack.last_mut() {
        under.inputs.clear();
        under.focus = None;
        // A particle's screen back from a list: its particle again from the live list.
        if let Kind2::Particle { data, pack: true, .. } = &mut under.kind
            && let Some(live) = glue::with(|st| st.particles.iter().find(|p| p.id == data.id).cloned())
        {
            **data = live;
        }
    }
    if s.stack.is_empty() {
        s.close = true;
    }
}

/// "Reset <title>": the screen's object back to its defaults (`resetConfig`), then `refresh`.
fn reset(s: &mut State) {
    let top = s.stack.len() - 1;
    match s.stack[top].kind.clone() {
        Kind2::Main => glue::with(|st| {
            st.config = Config::default();
            st.particles = st.pack.clone();
        }),
        Kind2::Section(sect) => glue::with(|st| {
            let d = Config::default();
            match sect {
                Sect::Perf => st.config.perf = d.perf,
                Sect::Sound => st.config.sound = d.sound,
                Sect::Wind => st.config.wind = d.wind,
                Sect::Compat => st.config.compat = d.compat,
                Sect::Streak => st.config.streak = d.streak,
            }
        }),
        Kind2::Particle { pack, .. } => {
            if let Kind2::Particle { data, .. } = &mut s.stack[top].kind {
                let default = if pack { glue::with(|st| st.pack.iter().find(|p| p.id == data.id).cloned()) } else { None };
                **data = default.unwrap_or_default();
                write_back(data, pack);
            }
        }
        // A list has no fields to reset.
        Kind2::List(_) | Kind2::Link(_) => {}
        Kind2::Object(obj) => {
            let default = match obj {
                ListObj::Ignored => Config::default().compat.weather_ignore_blocks,
                _ => {
                    let id = particle_page(&mut s.stack, top).map(|(d, _)| d.id.clone());
                    let pack = glue::with(|st| id.and_then(|id| st.pack.iter().find(|p| p.id == id).cloned())).unwrap_or_default();
                    if obj == ListObj::Biomes { pack.biome_list } else { pack.block_list }
                }
            };
            with_object(&mut s.stack, top, obj, |w| *w = default);
        }
    }
    refresh(s);
}

/// A button's click.
fn act(s: &mut State, a: &Act, back: bool) {
    let top = s.stack.len() - 1;
    match a {
        Act::Open(Kind2Key::Section(sect)) => push(s, Kind2::Section(*sect)),
        Act::Open(Kind2Key::List(of)) => push(s, Kind2::List(*of)),
        Act::Open(Kind2Key::Object(obj)) => push(s, Kind2::Object(*obj)),
        Act::Toggle(i) => glue::with(|st| {
            if let Some(p) = st.particles.get_mut(*i) {
                p.enabled = !p.enabled;
            }
        }),
        Act::Edit(i) => {
            let Some((data, pack)) = glue::with(|st| {
                let data = st.particles.get(*i).cloned()?;
                let pack = st.pack.iter().any(|p| p.id == data.id);
                // The player's own particle leaves the list while it is edited (its id may change).
                if !pack {
                    st.particles.remove(*i);
                }
                Some((data, pack))
            }) else {
                return;
            };
            let title = t(&format!("particlerain.{}", data.id));
            push(s, Kind2::Particle { data: Box::new(data), pack, title });
        }
        Act::Remove(i) => {
            glue::with(|st| {
                if *i < st.particles.len() {
                    st.particles.remove(*i);
                }
            });
            refresh(s);
        }
        Act::AddParticle => push(s, Kind2::Particle { data: Box::default(), pack: false, title: t("particlerain.addNew") }),
        Act::Cycle(key) => {
            let v = value_of(s, key);
            let next = match (s.stack[top].kind.clone(), v) {
                (Kind2::Object(obj), _) if key == "isWhitelist" => {
                    let cur = with_object(&mut s.stack, top, obj, |w| w.is_whitelist).unwrap_or(true);
                    Val::Bool(!cur)
                }
                (_, Val::Bool(b)) => Val::Bool(!b),
                (_, Val::Enum(i)) => {
                    let n = enum_kind_of(s, key).map_or(1, |k| k.values().len()) as u8;
                    Val::Enum(if back { (i + n - 1) % n } else { (i + 1) % n })
                }
                _ => return,
            };
            set_value(s, key, next);
        }
        Act::ListRemove(i) => {
            let i = *i;
            match s.stack[top].kind.clone() {
                Kind2::List(of) => {
                    if let Some((d, pack)) = particle_page(&mut s.stack, top) {
                        match of {
                            ListOf::Precipitation if i < d.precipitation.len() => {
                                d.precipitation.remove(i);
                            }
                            ListOf::Sprites if i < d.sprite_locations.len() => {
                                d.sprite_locations.remove(i);
                            }
                            _ => {}
                        }
                        write_back(d, pack);
                    }
                }
                Kind2::Object(obj) => {
                    with_object(&mut s.stack, top, obj, |w| {
                        if i < w.entries.len() {
                            w.entries.remove(i);
                        }
                    });
                }
                _ => {}
            }
            refresh(s);
        }
        Act::ListAdd => {
            match s.stack[top].kind.clone() {
                Kind2::List(of) => {
                    if let Some((d, pack)) = particle_page(&mut s.stack, top) {
                        match of {
                            ListOf::Precipitation => d.precipitation.push(core::Precipitation::None),
                            ListOf::Sprites => d.sprite_locations.push(String::new()),
                        }
                        write_back(d, pack);
                    }
                }
                Kind2::Object(obj) => {
                    with_object(&mut s.stack, top, obj, |w| w.entries.push(String::new()));
                }
                _ => {}
            }
            refresh(s);
        }
        Act::Link(url) => push(s, Kind2::Link(url.clone())),
    }
}

fn enum_kind_of(s: &State, key: &str) -> Option<EnumKind> {
    match &s.stack.last()?.kind {
        Kind2::Particle { .. } => Field::ALL.iter().find(|f| format!("{f:?}") == key).and_then(|f| match field_kind(*f) {
            Kind::Enum(k) => Some(k),
            _ => None,
        }),
        Kind2::List(ListOf::Precipitation) => Some(EnumKind::Precipitation),
        _ => None,
    }
}

/// A slider moved to `pos` (0..1): `applyValue`, the value in its steps.
fn slide(s: &mut State, key: &str, pos: f32) {
    let step = match &s.stack.last().map(|p| p.kind.clone()) {
        Some(Kind2::Section(sect)) => sect.options().iter().find(|o| format!("{o:?}") == key).and_then(|o| match o.kind() {
            Kind::Slider(step, _) => Some(step),
            _ => None,
        }),
        Some(Kind2::Particle { .. }) => Field::ALL.iter().find(|f| format!("{f:?}") == key).and_then(|f| match field_kind(*f) {
            Kind::Slider(step, _) => Some(step),
            _ => None,
        }),
        _ => None,
    };
    let Some(step) = step else { return };
    let v = (pos / step).round() * step;
    set_value(s, key, Val::Float(v));
    SLIDERS.with(|m| {
        m.borrow_mut().insert(key.to_owned(), pos);
    });
}

thread_local! {
    /// Each slider's own position while it lives (`AbstractSliderButton.value`), which its message
    /// shows unrounded.
    static SLIDERS: RefCell<HashMap<String, f32>> = RefCell::new(HashMap::new());
}

/// The text typed into an input: its value when it reads as one (`setResponder`), else red.
fn typed(s: &mut State, key: &str) {
    let top = s.stack.len() - 1;
    let Some(text) = s.stack[top].inputs.get(key).map(|i| i.edit.value.clone()) else { return };
    let current = value_of(s, key);
    let new = match current {
        Val::Int(_) => parse_int(&text).map(Val::Int),
        Val::Float(_) => parse_float(&text).map(Val::Float),
        Val::Str(_) => Some(Val::Str(text)),
        Val::Color(_) => core::decode_color(&text).map(Val::Color),
        _ => None,
    };
    let valid = new.is_some();
    if let Some(v) = new {
        set_value(s, key, v);
    }
    if let Some(i) = s.stack.last_mut().and_then(|p| p.inputs.get_mut(key)) {
        i.valid = valid;
    }
}

// ---- layout -------------------------------------------------------------------------------------

/// `HeaderAndFooterLayout`'s list: between the 33-pixel header and footer.
fn list_bounds(h: f32) -> (f32, f32) {
    (33.0, h - 33.0)
}

fn left(w: f32) -> f32 {
    ((w as i32) / 2 - 155) as f32
}

fn content_height(rows: usize) -> f32 {
    rows as f32 * ROW + 4.0
}

fn max_scroll(rows: usize, h: f32) -> f32 {
    let (top, bottom) = list_bounds(h);
    (content_height(rows) - (bottom - top)).max(0.0)
}

/// Each cell's rectangle, scrolled.
fn cell_rects(rows: &[Row], w: f32, scroll: f32) -> Vec<(usize, usize, [f32; 4])> {
    let x0 = left(w);
    let mut out = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        let y = 33.0 + 4.0 + ROW * r as f32 - scroll.floor();
        for (c, p) in row.iter().enumerate() {
            out.push((r, c, [x0 + p.x, y, p.w, BUTTON_HEIGHT]));
        }
    }
    out
}

/// The footer's Reset and Done (a `GridLayout` of two default buttons 8 apart).
fn footer(w: f32, h: f32) -> ([f32; 4], [f32; 4]) {
    let x = ((w - 308.0) * 0.5) as i32 as f32;
    let y = h - 33.0 + 6.0;
    ([x, y, 150.0, 20.0], [x + 158.0, y, 150.0, 20.0])
}

fn inside(p: (f32, f32), r: [f32; 4]) -> bool {
    ui::hovered(p, r[0], r[1], r[2], r[3])
}

fn scrollbar_x(w: f32) -> f32 {
    (w / 2.0).floor() + 163.0
}

/// `ConfirmLinkScreen` (trusted): the question, the link, then Open, Copy and Cancel.
struct LinkLayout {
    lines: Vec<(Text, f32, f32)>,
    buttons: [(f32, f32); 3],
}

fn link_layout(atlas: &HudAtlas, (w, h): (f32, f32), url: &str) -> LinkLayout {
    let title = Text::translate("chat.link.confirmTrusted", &[]);
    let message: Vec<Text> = atlas.font.split_lines(&Text::literal(url.to_owned()), w - 50.0).into_iter().take(15).map(|(l, _)| l).collect();
    let mh = 9.0 * message.len() as f32;
    let total = 9.0 + 8.0 + mh + 8.0 + 16.0 + 20.0;
    let mut y = ((h - total) / 2.0).trunc();
    let mut lines = Vec::new();
    let mut add = |line: &Text, at: f32| {
        let lw = atlas.font.width_text(line);
        lines.push((line.clone(), ((w - lw) / 2.0).trunc(), at));
    };
    add(&title, y);
    y += 17.0;
    for line in &message {
        add(line, y);
        y += 9.0;
    }
    let by = y + 8.0 + 16.0;
    let bx = ((w - 308.0) / 2.0).trunc();
    LinkLayout { lines, buttons: [(bx, by), (bx + 104.0, by), (bx + 208.0, by)] }
}

const LINK_LABELS: [&str; 3] = ["chat.link.open", "gui.copy_link_to_clipboard", "gui.cancel"];

// ---- drawing ------------------------------------------------------------------------------------

fn in_world(game: &Game<'_>, mode: Mode) -> bool {
    match mode {
        Mode::World => true,
        Mode::Page { .. } => game.ui.options.in_world,
    }
}

fn tooltip_lines(atlas: &HudAtlas, text: &str) -> Vec<Text> {
    atlas.font.split_lines(&Text::literal(text.to_owned()), 170.0).into_iter().map(|(l, _)| l).collect()
}

fn argb(c: u32) -> u32 {
    rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8)
}

pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !shown(game, screen) {
        return;
    }
    let atlas = &game.gui.atlas;
    let (w, h) = (batch.width, batch.height);
    let mouse = (screen.mouse[0], screen.mouse[1]);
    with(|s| {
        let world = in_world(game, s.mode);
        if world {
            game.native_apply_screen_blur();
        }
        if let Some(Kind2::Link(url)) = s.stack.last().map(|p| p.kind.clone()) {
            ui::menu_background(batch, atlas, 0.0, 0.0, w, h, world);
            let layout = link_layout(atlas, (w, h), &url);
            for (line, x, y) in &layout.lines {
                batch.text_styled(atlas, line, *x, *y, u32::MAX, true);
            }
            for ((x, y), key) in layout.buttons.iter().zip(LINK_LABELS) {
                ui::button(batch, atlas, *x, *y, 100.0, 20.0, &Text::translate(key, &[]), true, ui::hovered(mouse, *x, *y, 100.0, 20.0));
            }
            return;
        }
        let (title, rows) = build(s);
        let (top, bottom) = list_bounds(h);
        let max = max_scroll(rows.len(), h);
        let page_scroll = {
            let p = s.stack.last_mut().expect("a screen");
            p.scroll = p.scroll.clamp(0.0, max);
            p.scroll
        };
        ui::menu_background(batch, atlas, 0.0, 0.0, w, h, world);
        ui::list_background(batch, atlas, top, bottom, world);
        let start = batch.vertices.len();
        let over_list = mouse.1 >= top && mouse.1 < bottom;
        let focus = s.stack.last().and_then(|p| p.focus.clone());
        let mut tooltip: Option<(Vec<Text>, [f32; 4])> = None;
        for (r, c, rect) in cell_rects(&rows, w, page_scroll) {
            if rect[1] + rect[3] < top || rect[1] > bottom {
                continue;
            }
            let p = &rows[r][c];
            let hover = over_list && inside(mouse, rect);
            let key = cell_key(r, c, &p.cell);
            let focused = focus.as_deref() == Some(key.as_str());
            match &p.cell {
                Cell::Label(text, align) => {
                    let tw = atlas.font.width_text(text);
                    if tw <= rect[2] {
                        let x = rect[0] + (align * (rect[2] - tw)).round();
                        batch.text_styled(atlas, text, x, rect[1] + 5.0, u32::MAX, true);
                    } else {
                        // Too long for its cell: clipped to it and slid back and forth, as the
                        // game's widgets show a long message (`renderScrollingString`).
                        ui::scrolling_text(batch, atlas, text, rect[0], rect[0] + rect[2], rect[1], rect[1] + rect[3], rect[1] + 5.0, u32::MAX);
                    }
                }
                Cell::Button(text, _) | Cell::Cycle(text, _) => ui::button(batch, atlas, rect[0], rect[1], rect[2], rect[3], text, true, hover || (focused && s.keyboard)),
                Cell::Slider(k, text, value) => {
                    let pos = SLIDERS.with(|m| m.borrow().get(k).copied()).unwrap_or(*value);
                    let text = slider_message(s, k, pos).unwrap_or_else(|| text.clone());
                    ui::slider_ext(batch, atlas, rect[0], rect[1], rect[2], &text, pos, hover, focused && s.keyboard, false, true, s.dragging.as_deref() == Some(k.as_str()));
                }
                Cell::Input(k, start_text, fmt) => {
                    let page = s.stack.last_mut().expect("a screen");
                    let input = page.inputs.entry(k.clone()).or_insert_with(|| {
                        let mut edit = EditBox::new(rect[0], rect[1], rect[2], rect[3]);
                        edit.max_length = 128;
                        edit.set_value(start_text);
                        edit.focused = false;
                        Input { edit, valid: true }
                    });
                    input.edit.x = rect[0];
                    input.edit.y = rect[1];
                    input.edit.width = rect[2];
                    input.edit.height = rect[3];
                    input.edit.focused = focused;
                    input.edit.text_color = if !focused {
                        0
                    } else if input.valid {
                        argb(TEXT)
                    } else {
                        argb(0xFFFF_5555)
                    };
                    input.edit.draw(batch, atlas, None);
                    let shown = formatted(*fmt, &input.edit.value);
                    let (tx, ty) = (rect[0] + 4.0, rect[1] + 6.0);
                    // The value's text stays inside its field.
                    let start = batch.vertices.len();
                    if focused {
                        batch.text_styled(atlas, &shown, tx, ty, argb(DARK), true);
                        batch.text_styled(atlas, &Text::literal(input.edit.value.clone()), tx, ty, argb(TEXT), true);
                    } else {
                        batch.text_styled(atlas, &shown, tx, ty, argb(TEXT), true);
                    }
                    batch.clip_since(start, rect[0] + 4.0, rect[1], rect[0] + rect[2] - 4.0, rect[1] + rect[3]);
                }
            }
            if hover && let Some(tip) = &p.tooltip {
                tooltip = Some((tooltip_lines(atlas, tip), rect));
            }
        }
        batch.clip_since(start, 0.0, top, w, bottom);
        ui::separators(batch, atlas, top, bottom, world);
        if content_height(rows.len()) > bottom - top {
            ui::scrollbar(batch, atlas, scrollbar_x(w), top, bottom, content_height(rows.len()), page_scroll);
        }
        let tw = atlas.font.width_text(&Text::literal(title.clone()));
        batch.text_styled(atlas, &Text::literal(title.clone()), ((w - tw) * 0.5) as i32 as f32, 12.0, u32::MAX, true);
        let (reset, done) = footer(w, h);
        let reset_text = Text::translate("controls.reset", &[]).append(Text::literal(format!(" {title}")));
        ui::button(batch, atlas, reset[0], reset[1], reset[2], reset[3], &reset_text, true, inside(mouse, reset) || (s.keyboard && focus.as_deref() == Some("reset")));
        ui::action_button(
            batch,
            atlas,
            horizon_client::widgets::Action::Done,
            done[0],
            done[1],
            done[2],
            done[3],
            &Text::translate("gui.done", &[]),
            true,
            inside(mouse, done) || (s.keyboard && focus.as_deref() == Some("done")),
        );
        if let Some((lines, rect)) = tooltip {
            ui::tooltip_menu(batch, atlas, &lines, mouse, rect);
        }
    });
}

/// A slider's message at its own position (`updateMessage`: the formatter of the raw value).
fn slider_message(s: &mut State, key: &str, pos: f32) -> Option<Text> {
    let (name, fmt) = match &s.stack.last()?.kind {
        Kind2::Section(sect) => sect.options().iter().find(|o| format!("{o:?}") == key).and_then(|o| match o.kind() {
            Kind::Slider(_, fmt) => Some((format!("particlerain.{}", o.name()), fmt)),
            _ => None,
        })?,
        Kind2::Particle { .. } => Field::ALL.iter().find(|f| format!("{f:?}") == key).and_then(|f| match field_kind(*f) {
            Kind::Slider(_, fmt) => Some((format!("particlerain.{}", f.name()), fmt)),
            _ => None,
        })?,
        _ => return None,
    };
    Some(name_value_plain(&t(&name), formatted(fmt, &java_text(pos))))
}

/// The key of a focusable cell.
fn cell_key(r: usize, c: usize, cell: &Cell) -> String {
    match cell {
        Cell::Slider(k, ..) | Cell::Input(k, ..) => k.clone(),
        _ => format!("cell{r}.{c}"),
    }
}

// ---- input --------------------------------------------------------------------------------------

fn size(screen: &NativeScreenView<'_>) -> (f32, f32) {
    (screen.rect[2].floor(), screen.rect[3].floor())
}

fn bit(button: i32) -> u8 {
    1 << button.clamp(0, 7)
}

/// The cell under the pointer in the list.
fn cell_at(s: &mut State, p: (f32, f32), w: f32, h: f32) -> Option<(Placed, [f32; 4], String)> {
    let (top, bottom) = list_bounds(h);
    if p.1 < top || p.1 >= bottom {
        return None;
    }
    let (_, rows) = build(s);
    let scroll = s.stack.last()?.scroll;
    let (r, c, rect) = cell_rects(&rows, w, scroll).into_iter().find(|(_, _, rect)| inside(p, *rect))?;
    let key = cell_key(r, c, &rows[r][c].cell);
    let mut rows = rows;
    Some((rows[r].swap_remove(c), rect, key))
}

pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32) -> bool {
    if !shown(game, screen) {
        return false;
    }
    let (w, h) = size(screen);
    let p = (screen.mouse[0], screen.mouse[1]);
    let atlas = &game.gui.atlas;
    with(|s| {
        s.taken |= bit(button);
        if button != 0 {
            return;
        }
        s.keyboard = false;
        if let Some(Kind2::Link(url)) = s.stack.last().map(|x| x.kind.clone()) {
            let layout = link_layout(atlas, (w, h), &url);
            for (i, (x, y)) in layout.buttons.iter().enumerate() {
                if ui::hovered(p, *x, *y, 100.0, 20.0) {
                    s.requests.push(Request::Click);
                    match i {
                        0 => s.requests.push(Request::OpenUrl(url.clone())),
                        1 => s.requests.push(Request::Copy(url.clone())),
                        _ => {}
                    }
                    pop(s);
                    return;
                }
            }
            return;
        }
        let (reset_rect, done_rect) = footer(w, h);
        if inside(p, done_rect) {
            s.requests.push(Request::Click);
            pop(s);
            return;
        }
        if inside(p, reset_rect) {
            s.requests.push(Request::Click);
            reset(s);
            return;
        }
        // The scrollbar.
        let (top, bottom) = list_bounds(h);
        let rows = build(s).1.len();
        let sx = scrollbar_x(w);
        if p.0 >= sx && p.0 <= sx + 6.0 && p.1 >= top && p.1 < bottom && max_scroll(rows, h) > 0.0 {
            s.scrolling = Some(p.1);
            return;
        }
        let Some((cell, rect, key)) = cell_at(s, p, w, h) else {
            if let Some(page) = s.stack.last_mut() {
                page.focus = None;
            }
            return;
        };
        if let Some(page) = s.stack.last_mut() {
            page.focus = (!matches!(cell.cell, Cell::Label(..))).then(|| key.clone());
        }
        match &cell.cell {
            Cell::Button(_, a) | Cell::Cycle(_, a) => {
                s.requests.push(Request::Click);
                act(s, a, modifiers & SHIFT != 0);
            }
            Cell::Slider(k, ..) => {
                s.dragging = Some(k.clone());
                slide(s, k, ui::slider_value(rect[0], rect[2], p.0));
            }
            Cell::Input(k, ..) => {
                if let Some(input) = s.stack.last_mut().and_then(|pg| pg.inputs.get_mut(k)) {
                    input.edit.focused = true;
                    input.edit.click(atlas, p.0, modifiers & SHIFT != 0, false);
                }
            }
            Cell::Label(..) => {}
        }
    });
    true
}

pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    let was = with(|s| {
        let was = s.taken & bit(button) != 0;
        s.taken &= !bit(button);
        if button == 0 {
            s.scrolling = None;
            // AbstractSliderButton.onRelease: the click once the handle is let go.
            if s.dragging.take().is_some() {
                s.requests.push(Request::Click);
            }
        }
        was
    })
    .unwrap_or(false);
    was || shown(game, screen)
}

pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    if !shown(game, screen) {
        return false;
    }
    let (w, h) = size(screen);
    let p = (screen.mouse[0], screen.mouse[1]);
    with(|s| {
        if let Some(last) = s.scrolling {
            let rows = build(s).1.len();
            let (top, bottom) = list_bounds(h);
            let max = max_scroll(rows, h);
            let height = bottom - top;
            let content = content_height(rows);
            let bar = (height * height / content).trunc().clamp(32.0, height - 8.0);
            if let Some(page) = s.stack.last_mut() {
                page.scroll = if p.1 < top {
                    0.0
                } else if p.1 > bottom {
                    max
                } else {
                    (page.scroll + (p.1 - last) * (max.max(1.0) / (height - bar)).max(1.0)).clamp(0.0, max)
                };
            }
            s.scrolling = Some(p.1);
            return;
        }
        if let Some(k) = s.dragging.clone() {
            let (_, rows) = build(s);
            let scroll = s.stack.last().map_or(0.0, |x| x.scroll);
            if let Some((_, _, rect)) = cell_rects(&rows, w, scroll).into_iter().find(|(r, c, _)| cell_key(*r, *c, &rows[*r][*c].cell) == k) {
                slide(s, &k, ui::slider_value(rect[0], rect[2], p.0));
            }
        }
        let focus = s.stack.last().and_then(|x| x.focus.clone());
        if let Some(f) = focus
            && let Some(input) = s.stack.last_mut().and_then(|pg| pg.inputs.get_mut(&f))
        {
            input.edit.drag_to(&game.gui.atlas, p.0);
        }
    });
    true
}

/// The wheel scrolls the list (`AbstractScrollArea.mouseScrolled`, half a row a step).
pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !shown(game, screen) {
        return false;
    }
    let (_, h) = size(screen);
    with(|s| {
        if matches!(s.stack.last().map(|x| &x.kind), Some(Kind2::Link(_))) {
            return;
        }
        let rows = build(s).1.len();
        let max = max_scroll(rows, h);
        if let Some(page) = s.stack.last_mut() {
            page.scroll = (page.scroll - scroll[1] as f32 * ROW / 2.0).clamp(0.0, max);
        }
    });
    true
}

/// The focusable cells of the top screen in Tab's order, then Reset and Done.
fn focus_order(s: &mut State) -> Vec<String> {
    let (_, rows) = build(s);
    let mut v = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        for (c, p) in row.iter().enumerate() {
            if !matches!(p.cell, Cell::Label(..)) {
                v.push(cell_key(r, c, &p.cell));
            }
        }
    }
    v.push("reset".into());
    v.push("done".into());
    v
}

pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !shown(game, screen) {
        return false;
    }
    let (control, shift) = (modifiers & CONTROL != 0, modifiers & SHIFT != 0);
    with(|s| {
        if key == "key.keyboard.escape" {
            pop(s);
            return;
        }
        if matches!(s.stack.last().map(|x| &x.kind), Some(Kind2::Link(_))) {
            return;
        }
        if key == "key.keyboard.tab" {
            s.keyboard = true;
            let all = focus_order(s);
            let cur = s.stack.last().and_then(|x| x.focus.clone());
            let i = cur.and_then(|f| all.iter().position(|k| *k == f));
            let n = all.len();
            let next = match i {
                None if shift => n - 1,
                None => 0,
                Some(i) if shift => (i + n - 1) % n,
                Some(i) => (i + 1) % n,
            };
            if let Some(page) = s.stack.last_mut() {
                page.focus = Some(all[next].clone());
            }
            return;
        }
        let Some(focus) = s.stack.last().and_then(|x| x.focus.clone()) else { return };
        let activate = matches!(key, "key.keyboard.enter" | "key.keyboard.space" | "key.keyboard.keypad.enter");
        if focus == "done" {
            if activate {
                s.requests.push(Request::Click);
                pop(s);
            }
            return;
        }
        if focus == "reset" {
            if activate {
                s.requests.push(Request::Click);
                reset(s);
            }
            return;
        }
        if s.stack.last().is_some_and(|x| x.inputs.contains_key(&focus)) {
            let edit = match key {
                "key.keyboard.v" if control => Some(Request::Paste),
                "key.keyboard.c" if control => s.stack.last().and_then(|x| x.inputs.get(&focus)).map(|i| Request::Copy(i.edit.selected_text())),
                "key.keyboard.x" if control => {
                    let text = s.stack.last_mut().and_then(|x| x.inputs.get_mut(&focus)).map(|i| {
                        let t = i.edit.selected_text();
                        i.edit.delete_chars(0);
                        t
                    });
                    typed(s, &focus);
                    text.map(Request::Copy)
                }
                _ => {
                    if let Some(code) = horizon_client::keybinds::key_code(key)
                        && let Some(i) = s.stack.last_mut().and_then(|x| x.inputs.get_mut(&focus))
                    {
                        let before = i.edit.value.clone();
                        i.edit.key(code, control, shift);
                        i.edit.reset_blink();
                        if i.edit.value != before {
                            typed(s, &focus);
                        }
                    }
                    None
                }
            };
            if let Some(e) = edit {
                s.requests.push(e);
            }
            return;
        }
        // A focused slider steps with the arrows; a button with Enter or Space.
        let (_, rows) = build(s);
        let Some(cell) = rows.iter().enumerate().flat_map(|(r, row)| row.iter().enumerate().map(move |(c, p)| (r, c, p))).find(|(r, c, p)| cell_key(*r, *c, &p.cell) == focus).map(|(_, _, p)| p.cell.clone()) else {
            return;
        };
        match cell {
            Cell::Slider(k, _, v) => {
                let step = match key {
                    "key.keyboard.left" => -1.0,
                    "key.keyboard.right" => 1.0,
                    _ => 0.0,
                };
                if step != 0.0 {
                    let pos = SLIDERS.with(|m| m.borrow().get(&k).copied()).unwrap_or(v);
                    slide(s, &k, (pos + step / (BIG_BUTTON_WIDTH - 8.0)).clamp(0.0, 1.0));
                }
            }
            Cell::Button(_, a) | Cell::Cycle(_, a) if activate => {
                s.requests.push(Request::Click);
                act(s, &a, shift);
            }
            _ => {}
        }
    });
    true
}

pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    shown(game, screen)
}

fn insert(s: &mut State, text: &str) {
    let Some(focus) = s.stack.last().and_then(|x| x.focus.clone()) else { return };
    let text: String = text.chars().filter(|c| !c.is_control() && *c != '§').collect();
    let changed = s.stack.last_mut().and_then(|x| x.inputs.get_mut(&focus)).map(|i| {
        i.edit.insert_text(&text);
        i.edit.reset_blink();
    });
    if changed.is_some() {
        typed(s, &focus);
    }
}

pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, ch: char) -> bool {
    if !shown(game, screen) {
        return false;
    }
    with(|s| {
        // The key that opened the screen types too: not in its first moment.
        if s.opened.elapsed().as_millis() >= 100 {
            insert(s, &ch.to_string());
        }
    });
    true
}

// ---- once a frame -------------------------------------------------------------------------------

/// What the screens asked for, and their closing (Done or Escape on the last, or the game taking
/// their place).
pub(super) fn frame(game: &mut Game<'_>) {
    if glue::take_open_screen() {
        open_in_world(game);
    }
    let Some(mode) = with(|s| s.mode) else { return };
    for r in with(|s| std::mem::take(&mut s.requests)).unwrap_or_default() {
        match r {
            Request::Click => game.audio.play_button_click(),
            Request::Paste => {
                let text = horizon_client::clipboard::Clipboard::get(&mut game.clipboard);
                with(|s| insert(s, &text));
            }
            Request::Copy(text) => {
                if !text.is_empty() {
                    horizon_client::clipboard::Clipboard::set(&mut game.clipboard, &text);
                }
            }
            Request::OpenUrl(url) => super::packs::open_uri(&url),
            Request::ClearParticles => {
                game.particles.clear();
                weather::clear();
            }
            Request::Reload => glue::reload_resources(game),
        }
    }
    let close = with(|s| s.close).unwrap_or(false);
    let gone = match mode {
        Mode::World => game.native_mod_screen() != Some(glue::ID),
        Mode::Page { row, .. } => options_row(game) != Some(row),
    };
    if !close && !gone {
        return;
    }
    // The game took the screens' place: each one left as `removed` does.
    if gone && !close {
        with(|s| {
            while !s.stack.is_empty() {
                pop(s);
            }
        });
        for r in with(|s| std::mem::take(&mut s.requests)).unwrap_or_default() {
            match r {
                Request::Reload => glue::reload_resources(game),
                Request::ClearParticles => {
                    game.particles.clear();
                    weather::clear();
                }
                _ => {}
            }
        }
    }
    STATE.with(|s| *s.borrow_mut() = None);
    SLIDERS.with(|m| m.borrow_mut().clear());
    if gone {
        return;
    }
    match mode {
        Mode::World => game.native_mod_screen_close(glue::ID),
        Mode::Page { row, modmenu: false } => game.native_settings_refresh(row),
        // Back to the parent, Mod Menu, the way Escape leaves the options.
        Mode::Page { modmenu: true, .. } => {
            let events = game.ui.options.back(&mut game.settings);
            let _ = game.handle_option_events(events);
        }
    }
}

/// A click on the mod's page, or Mod Menu's Configure: the screens.
pub(super) fn page_click(game: &Game<'_>, mod_id: &str, button: usize) {
    if mod_id != glue::ID {
        return;
    }
    if button == 0 || button == MODMENU_CONFIGURE {
        open_over_options(game, button != 0);
    }
}
