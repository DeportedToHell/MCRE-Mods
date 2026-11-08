// LambDynamicLights for Horizon: a Rust port of LambDynamicLights by
// LambdAurora (https://github.com/LambdAurora/LambDynamicLights, branch 26.3,
// version 4.13.0), under the Lambda License (LICENSE). Items that give light
// light up the world around whoever holds them, and glowing mobs, burning
// entities, fireballs, lit TNT, swelling creepers, beacon beams and guardian
// lasers do too.
//
// Changes from the original: rewritten in Rust as a Horizon native mod. The
// light reaches the world through Horizon's own dynamic light shader (the 16
// sources nearest the camera, one level less per block) instead of chunk
// rebuilds, so a source's level and height are fitted to look like the
// original's falloff (15 levels over 7.75 blocks), see `Reach`; the light source data the original reads from resource packs is
// the original's own, built in; fireflies, the warden's sonic boom, display
// entities and the performance options (adaptive ticking, chunk rebuild
// strategy, debug renderers) are not ported.
//
// This file is both the mod's Cargo library (its tests) and the module the
// bundle adds to the game (`game::lambdynamiclights`). It holds no game types:
// `glue.rs` (`game::lambdynamiclights_glue`) reads the game.

/// The mod's id (its settings folder), name, version and command.
pub const ID: &str = "lambdynamiclights";
pub const NAME: &str = "LambDynamicLights";
pub const VERSION: &str = "1.1.0";
pub const COMMAND: &str = "lambdynamiclights";
/// The original's mod id, taken as the command too.
pub const SHORT_COMMAND: &str = "lambdynlights";
pub const DESCRIPTION: &str = "Dynamic lights by LambdAurora: held and dropped light sources, glowing mobs, fireballs, TNT, beacon beams. /lambdynamiclights opens the settings.";

/// The original's `DynamicLightsMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Off,
    Fastest,
    Fast,
    Fancy,
}

impl Mode {
    /// `TickMode.delay`: a source refreshes every this many ticks (Fancy: every frame).
    pub fn delay(self) -> u32 {
        match self {
            Mode::Fastest => 10,
            Mode::Fast => 5,
            Mode::Off | Mode::Fancy => 1,
        }
    }
}

/// `ExplosiveLightingMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Explosive {
    Off,
    Simple,
    Fancy,
}

/// How far the light reaches. The original's light falls 15 levels over 7.75
/// blocks; Horizon's dynamic light shader takes one level off per block and a
/// mod can't change that, so a source is drawn dimmer and lower to look like
/// the original's: `Original` is the closest fit (least squares of the
/// lightmap's brightness on the ground around a source, in caves and at
/// night, for levels 8 to 15), `Short` reaches about as far as the original
/// but is dimmer near the source, `Horizon` is Horizon's own reach.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reach {
    Short,
    Original,
    Horizon,
}

impl Reach {
    /// The level the shader gets for a source of `level`.
    pub fn scale(self) -> f32 {
        match self {
            Reach::Short => 0.55,
            Reach::Original => 0.72,
            Reach::Horizon => 1.0,
        }
    }
    /// Where a living entity's light sits over its feet: low, so the ground
    /// around it is lit as the original lights it (the original's is at the eyes).
    pub fn height(self, eye: f64) -> f64 {
        match self {
            Reach::Horizon => eye,
            Reach::Short | Reach::Original => eye.min(0.5),
        }
    }
}

const REACHES: [Reach; 3] = [Reach::Short, Reach::Original, Reach::Horizon];

fn reach_name(r: Reach) -> &'static str {
    ["short", "original", "horizon"][REACHES.iter().position(|x| *x == r).unwrap_or(1)]
}

/// Lights the shader reads (`dynamic_light::MAX`) and how far they may be.
pub const MAX_LIGHTS: usize = 16;
const REACH_BLOCKS: f64 = 96.0;

/// `dynamic_light::select_iter` with this mod's levels: the `MAX_LIGHTS`
/// sources nearest `camera` within 96 blocks (the first ones win ties), as
/// the shader reads them: camera-relative, at `level * reach.scale()`.
pub fn select(sources: &[([f64; 3], u8)], camera: [f64; 3], reach: Reach) -> Vec<[f32; 4]> {
    let mut near: Vec<(f64, [f64; 3], u8)> = sources
        .iter()
        .filter(|(_, level)| *level > 0)
        .map(|(at, level)| {
            let d = (0..3).map(|i| (at[i] - camera[i]).powi(2)).sum::<f64>();
            (d, *at, *level)
        })
        .filter(|(d, _, _)| (0.0..=REACH_BLOCKS * REACH_BLOCKS).contains(d))
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    near.into_iter()
        .take(MAX_LIGHTS)
        .map(|(_, at, level)| {
            [
                (at[0] - camera[0]) as f32,
                (at[1] - camera[1]) as f32,
                (at[2] - camera[2]) as f32,
                f32::from(level.min(15)) * reach.scale(),
            ]
        })
        .collect()
}

const MODES: [Mode; 4] = [Mode::Off, Mode::Fastest, Mode::Fast, Mode::Fancy];
const EXPLOSIVES: [Explosive; 3] = [Explosive::Off, Explosive::Simple, Explosive::Fancy];

/// Entities with a switch of their own on the settings page (the original
/// lists every entity type; any other is switched with the command).
pub const ENTITY_TOGGLES: [(&str, &str, &str); 19] = [
    ("minecraft:player", "Players", "Игроки"),
    ("minecraft:allay", "Allay", "Тихоня"),
    ("minecraft:blaze", "Blaze", "Ифрит"),
    ("minecraft:glow_squid", "Glow Squid", "Светящийся спрут"),
    ("minecraft:magma_cube", "Magma Cube", "Магмовый куб"),
    ("minecraft:creeper", "Creeper", "Крипер"),
    ("minecraft:enderman", "Enderman", "Эндермен"),
    ("minecraft:guardian", "Guardian", "Страж"),
    (
        "minecraft:elder_guardian",
        "Elder Guardian",
        "Древний страж",
    ),
    ("minecraft:item", "Item", "Предмет"),
    ("minecraft:item_frame", "Item Frame", "Рамка"),
    (
        "minecraft:glow_item_frame",
        "Glow Item Frame",
        "Светящаяся рамка",
    ),
    ("minecraft:fireball", "Fireball", "Огненный шар"),
    (
        "minecraft:small_fireball",
        "Small Fireball",
        "Малый огненный шар",
    ),
    (
        "minecraft:dragon_fireball",
        "Dragon Fireball",
        "Огненный шар дракона",
    ),
    ("minecraft:wither_skull", "Wither Skull", "Череп иссушителя"),
    (
        "minecraft:spectral_arrow",
        "Spectral Arrow",
        "Спектральная стрела",
    ),
    ("minecraft:arrow", "Arrow", "Стрела"),
    ("minecraft:tnt", "TNT", "Динамит"),
];

/// The options, as the original's `lambdynlights.toml` has them.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub mode: Mode,
    pub reach: Reach,
    pub self_light: bool,
    pub entities: bool,
    pub water_sensitive: bool,
    pub creeper: Explosive,
    pub tnt: Explosive,
    pub beam: bool,
    pub guardian_laser: bool,
    pub glowing_effect: bool,
    /// Entity types whose light is off (`light_sources.settings.entities`).
    pub disabled: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            mode: Mode::Fancy,
            reach: Reach::Original,
            self_light: true,
            entities: true,
            water_sensitive: true,
            creeper: Explosive::Simple,
            tnt: Explosive::Off,
            beam: true,
            guardian_laser: true,
            glowing_effect: true,
            // The original's default file turns the glow item frame off.
            disabled: vec!["minecraft:glow_item_frame".to_owned()],
        }
    }
}

/// The settings page's buttons, in order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Setting {
    Mode,
    Reach,
    SelfLight,
    Entities,
    WaterSensitive,
    Creeper,
    Tnt,
    Beam,
    GuardianLaser,
    GlowingEffect,
    Entity(usize),
    Reset,
}

/// The page: the options, a switch per listed entity, then the reset.
pub fn page() -> Vec<Setting> {
    let mut out = vec![
        Setting::Mode,
        Setting::Reach,
        Setting::SelfLight,
        Setting::Entities,
        Setting::WaterSensitive,
        Setting::Creeper,
        Setting::Tnt,
        Setting::Beam,
        Setting::GuardianLaser,
        Setting::GlowingEffect,
    ];
    out.extend((0..ENTITY_TOGGLES.len()).map(Setting::Entity));
    out.push(Setting::Reset);
    out
}

/// The files the options are kept in (one per key in the mod's settings folder).
pub const KEYS: [&str; 11] = [
    "mode",
    "horizon.reach",
    "light_sources.self",
    "light_sources.entities",
    "light_sources.water_sensitive_check",
    "light_sources.creeper",
    "light_sources.tnt",
    "light_sources.beam",
    "light_sources.guardian_laser",
    "light_sources.glowing_effect",
    "light_sources.disabled_entities",
];

fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "on" | "1" => Some(true),
        "false" | "off" | "0" => Some(false),
        _ => None,
    }
}

fn mode_name(m: Mode) -> &'static str {
    ["off", "fastest", "fast", "fancy"][MODES.iter().position(|x| *x == m).unwrap_or(3)]
}

fn explosive_name(e: Explosive) -> &'static str {
    ["off", "simple", "fancy"][EXPLOSIVES.iter().position(|x| *x == e).unwrap_or(0)]
}

/// An entity type id with its namespace.
pub fn full_id(id: &str) -> String {
    let id = id.trim().to_ascii_lowercase();
    if id.contains(':') {
        id
    } else {
        format!("minecraft:{id}")
    }
}

impl Config {
    /// The options as stored, read through `get`; a missing or unreadable
    /// value keeps its default.
    pub fn load(mut get: impl FnMut(&str) -> Option<String>) -> Config {
        let mut c = Config::default();
        for key in KEYS {
            if let Some(v) = get(key) {
                c.set(key, &v);
            }
        }
        c
    }

    /// Reads one stored value; false when it doesn't parse.
    pub fn set(&mut self, key: &str, v: &str) -> bool {
        let b = parse_bool(v);
        let lower = v.trim().to_ascii_lowercase();
        let ok = match key {
            "mode" => MODES
                .iter()
                .find(|m| mode_name(**m) == lower)
                .map(|m| self.mode = *m),
            "horizon.reach" => REACHES
                .iter()
                .find(|r| reach_name(**r) == lower)
                .map(|r| self.reach = *r),
            "light_sources.self" => b.map(|b| self.self_light = b),
            "light_sources.entities" => b.map(|b| self.entities = b),
            "light_sources.water_sensitive_check" => b.map(|b| self.water_sensitive = b),
            "light_sources.creeper" => EXPLOSIVES
                .iter()
                .find(|e| explosive_name(**e) == lower)
                .map(|e| self.creeper = *e),
            "light_sources.tnt" => EXPLOSIVES
                .iter()
                .find(|e| explosive_name(**e) == lower)
                .map(|e| self.tnt = *e),
            "light_sources.beam" => b.map(|b| self.beam = b),
            "light_sources.guardian_laser" => b.map(|b| self.guardian_laser = b),
            "light_sources.glowing_effect" => b.map(|b| self.glowing_effect = b),
            "light_sources.disabled_entities" => {
                self.disabled = v
                    .split([',', '\n'])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(full_id)
                    .collect();
                Some(())
            }
            _ => None,
        };
        ok.is_some()
    }

    /// The option's value as stored.
    pub fn value(&self, key: &str) -> String {
        let b = |v: bool| if v { "true" } else { "false" }.to_owned();
        match key {
            "mode" => mode_name(self.mode).to_owned(),
            "horizon.reach" => reach_name(self.reach).to_owned(),
            "light_sources.self" => b(self.self_light),
            "light_sources.entities" => b(self.entities),
            "light_sources.water_sensitive_check" => b(self.water_sensitive),
            "light_sources.creeper" => explosive_name(self.creeper).to_owned(),
            "light_sources.tnt" => explosive_name(self.tnt).to_owned(),
            "light_sources.beam" => b(self.beam),
            "light_sources.guardian_laser" => b(self.guardian_laser),
            "light_sources.glowing_effect" => b(self.glowing_effect),
            "light_sources.disabled_entities" => self.disabled.join(","),
            _ => String::new(),
        }
    }

    /// Whether entities of type `id` give light (`DynamicLightingEngine.canLightUp`):
    /// this client's player with the first-person option, every other entity
    /// with the entities option, and neither when its type is switched off.
    pub fn can_light_up(&self, id: &str, local_player: bool) -> bool {
        let on = if local_player {
            self.self_light
        } else {
            self.entities
        };
        on && !self.disabled.iter().any(|d| d == id)
    }

    /// Switches entity type `id`'s light; false when it already was so.
    pub fn set_entity(&mut self, id: &str, on: bool) -> bool {
        let id = full_id(id);
        let off = self.disabled.contains(&id);
        if on == !off {
            return false;
        }
        if on {
            self.disabled.retain(|d| *d != id);
        } else {
            self.disabled.push(id);
        }
        true
    }

    /// The button's label, in Russian when `ru`.
    pub fn label(&self, s: Setting, ru: bool) -> String {
        let pick = |en: &str, rus: &str| if ru { rus.to_owned() } else { en.to_owned() };
        let on = |v: bool| match (v, ru) {
            (true, false) => "ON".to_owned(),
            (false, false) => "OFF".to_owned(),
            (true, true) => "Вкл".to_owned(),
            (false, true) => "Выкл".to_owned(),
        };
        let explosive = |e: Explosive| match e {
            Explosive::Off => on(false),
            Explosive::Simple => pick("Simple", "Простое"),
            Explosive::Fancy => pick("Fancy", "Детальное"),
        };
        let (caption, value) = match s {
            Setting::Mode => (
                pick("Dynamic Lights", "Динамическое освещение"),
                match self.mode {
                    Mode::Off => on(false),
                    Mode::Fastest => pick("Fastest", "Быстрейшее"),
                    Mode::Fast => pick("Fast", "Быстрое"),
                    Mode::Fancy => pick("Fancy", "Детальное"),
                },
            ),
            Setting::Reach => (
                pick("Light Reach", "Дальность света"),
                match self.reach {
                    Reach::Short => pick("Shorter", "Короче"),
                    Reach::Original => pick("As the Original", "Как в оригинале"),
                    Reach::Horizon => pick("Horizon's", "Как в Horizon"),
                },
            ),
            Setting::SelfLight => (pick("First-Person", "От первого лица"), on(self.self_light)),
            Setting::Entities => (pick("Entities", "Сущности"), on(self.entities)),
            Setting::WaterSensitive => (
                pick(
                    "Water Sensitivity Check",
                    "Проверка чувствительности к воде",
                ),
                on(self.water_sensitive),
            ),
            Setting::Creeper => (pick("Creeper", "Крипер"), explosive(self.creeper)),
            Setting::Tnt => (pick("TNT", "Динамит"), explosive(self.tnt)),
            Setting::Beam => (pick("Beacon Beams", "Лучи маяков"), on(self.beam)),
            Setting::GuardianLaser => (
                pick("Guardian Lasers", "Лазеры стражей"),
                on(self.guardian_laser),
            ),
            Setting::GlowingEffect => (
                pick("Glowing Effect", "Эффект свечения"),
                on(self.glowing_effect),
            ),
            Setting::Entity(i) => {
                let (id, en, rus) = ENTITY_TOGGLES[i.min(ENTITY_TOGGLES.len() - 1)];
                let off = self.disabled.iter().any(|d| d == id);
                (
                    format!("{}: {}", pick("Light", "Свет"), pick(en, rus)),
                    on(!off),
                )
            }
            Setting::Reset => return pick("Reset to Defaults", "Сбросить настройки"),
        };
        format!("{caption}: {value}")
    }

    /// A click on the option's button: its next value (the previous one
    /// `back`). The keys it changed, to keep.
    pub fn click(&mut self, s: Setting, back: bool) -> Vec<&'static str> {
        let cycle = |i: usize, n: usize| if back { (i + n - 1) % n } else { (i + 1) % n };
        match s {
            Setting::Mode => {
                let i = MODES.iter().position(|m| *m == self.mode).unwrap_or(3);
                self.mode = MODES[cycle(i, 4)];
                vec!["mode"]
            }
            Setting::Reach => {
                let i = REACHES.iter().position(|r| *r == self.reach).unwrap_or(1);
                self.reach = REACHES[cycle(i, 3)];
                vec!["horizon.reach"]
            }
            Setting::SelfLight => {
                self.self_light ^= true;
                vec!["light_sources.self"]
            }
            Setting::Entities => {
                self.entities ^= true;
                vec!["light_sources.entities"]
            }
            Setting::WaterSensitive => {
                self.water_sensitive ^= true;
                vec!["light_sources.water_sensitive_check"]
            }
            Setting::Creeper => {
                let i = EXPLOSIVES
                    .iter()
                    .position(|e| *e == self.creeper)
                    .unwrap_or(0);
                self.creeper = EXPLOSIVES[cycle(i, 3)];
                vec!["light_sources.creeper"]
            }
            Setting::Tnt => {
                let i = EXPLOSIVES.iter().position(|e| *e == self.tnt).unwrap_or(0);
                self.tnt = EXPLOSIVES[cycle(i, 3)];
                vec!["light_sources.tnt"]
            }
            Setting::Beam => {
                self.beam ^= true;
                vec!["light_sources.beam"]
            }
            Setting::GuardianLaser => {
                self.guardian_laser ^= true;
                vec!["light_sources.guardian_laser"]
            }
            Setting::GlowingEffect => {
                self.glowing_effect ^= true;
                vec!["light_sources.glowing_effect"]
            }
            Setting::Entity(i) => {
                let id = ENTITY_TOGGLES[i.min(ENTITY_TOGGLES.len() - 1)].0;
                let off = self.disabled.iter().any(|d| d == id);
                self.set_entity(id, off);
                vec!["light_sources.disabled_entities"]
            }
            Setting::Reset => {
                *self = Config::default();
                KEYS.to_vec()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Item light (`ItemLightSources`, the original's `dynamiclights/item/*.json`)
// ---------------------------------------------------------------------------

/// An item light source: a fixed luminance or its block's, and whether it
/// goes out under water.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ItemRule {
    pub luminance: Option<u8>,
    pub water_sensitive: bool,
}

/// The original's item light sources; any other item glows as its block does.
pub fn item_rule(item: &str) -> Option<ItemRule> {
    let fixed = |l: u8, ws: bool| {
        Some(ItemRule {
            luminance: Some(l),
            water_sensitive: ws,
        })
    };
    let block = |ws: bool| {
        Some(ItemRule {
            luminance: None,
            water_sensitive: ws,
        })
    };
    match item {
        "minecraft:blaze_powder" | "minecraft:blaze_rod" => fixed(10, false),
        "minecraft:campfire" | "minecraft:soul_campfire" => block(true),
        "minecraft:fire_charge" => fixed(10, true),
        "minecraft:glow_berries" => fixed(14, false),
        "minecraft:glow_ink_sac" => fixed(12, false),
        "minecraft:glowstone_dust" => fixed(10, false),
        "minecraft:lava_bucket" => fixed(15, true),
        "minecraft:nether_star" => fixed(8, false),
        "minecraft:prismarine_crystals" => fixed(10, false),
        "minecraft:spectral_arrow" => fixed(8, false),
        "minecraft:torch"
        | "minecraft:redstone_torch"
        | "minecraft:soul_torch"
        | "minecraft:copper_torch" => block(true),
        _ => None,
    }
}

/// `ItemLightSources.getLuminance`: the item's light, `block` being its
/// block's (`getLightEmission` of the default state); a water-sensitive
/// source gives none `submerged`, with the check on.
pub fn item_luminance(cfg: &Config, item: &str, submerged: bool, block: u8) -> u8 {
    match item_rule(item) {
        Some(rule) if submerged && cfg.water_sensitive && rule.water_sensitive => 0,
        Some(rule) => rule.luminance.unwrap_or(block),
        None => block,
    }
    .min(15)
}

// ---------------------------------------------------------------------------
// Entity light (`EntityLightSources`, the original's `dynamiclights/entity/*.json`)
// ---------------------------------------------------------------------------

/// `LivingEntityMixin.dynamicLightTick`: fire or the glowing effect give
/// 15, else the brightest equipped item.
pub fn living(cfg: &Config, on_fire: bool, glowing: bool, items: u8) -> u8 {
    if on_fire || (cfg.glowing_effect && glowing) {
        15
    } else {
        items.min(15)
    }
}

/// What a mob's own light depends on, by species.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MobLight {
    None,
    /// `allay.json`: 8.
    Allay,
    /// `blaze.json`: 10 dry, 4 in water or rain.
    Blaze {
        wet: bool,
    },
    /// `GlowSquidLuminance`: dims for the dark ticks a hit gave it.
    GlowSquid {
        dark_ticks: i32,
    },
    /// `magma_cube.json` (`cube_mob_squish`): 11 squished past 0.6, else 8.
    MagmaCube {
        squish: f32,
    },
    /// `CreeperLuminance`: `getSwelling(0)`.
    Creeper {
        swelling: f32,
    },
    /// `EndermanLuminance`: its carried block's light.
    Enderman {
        carried: u8,
    },
}

/// The species' light, by the original's rules.
pub fn mob_luminance(cfg: &Config, light: MobLight) -> u8 {
    match light {
        MobLight::None => 0,
        MobLight::Allay => 8,
        MobLight::Blaze { wet } => {
            if wet {
                4
            } else {
                10
            }
        }
        MobLight::GlowSquid { dark_ticks } => {
            // Mth.clampedLerp(0, 12, 1 - dark / 10), then (int).
            let t = 1.0 - dark_ticks as f32 / 10.0;
            (12.0 * t.clamp(0.0, 1.0)) as u8
        }
        MobLight::MagmaCube { squish } => {
            if squish > 0.6 {
                11
            } else {
                8
            }
        }
        MobLight::Creeper { swelling } => {
            if swelling > 0.001 {
                match cfg.creeper {
                    Explosive::Off => 0,
                    Explosive::Simple => 10,
                    Explosive::Fancy => (swelling * 10.0).clamp(0.0, 15.0) as u8,
                }
            } else {
                0
            }
        }
        MobLight::Enderman { carried } => carried.min(15),
    }
}

/// `Creeper.getSwelling(0)`: the last tick's swell over `maxSwell - 2`.
pub fn creeper_swelling(previous_swell: i32, max_swell: i32) -> f32 {
    let span = (max_swell - 2).max(1);
    previous_swell as f32 / span as f32
}

/// The `Mth.smoothstep` the original's TNT fades with.
fn smoothstep(x: f32) -> f32 {
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

/// `ExplosiveLuminance`: lit TNT's light; none with the TNT option off.
pub fn tnt_luminance(cfg: &Config, fuse: i32, start_fuse: i32) -> u8 {
    match cfg.tnt {
        Explosive::Off => 0,
        Explosive::Simple => 10,
        Explosive::Fancy => {
            let t = if start_fuse > 0 {
                (fuse as f32 / start_fuse as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            ((-smoothstep(t) * 10.0) as i32 + 10).clamp(0, 15) as u8
        }
    }
}

/// Projectiles with their own light (`fireball.json`, `small_fireball.json`,
/// `dragon_fireball.json`, `wither_skull.json`): the fireballs go out under water.
pub fn projectile_luminance(id: &str, submerged: bool) -> u8 {
    match id {
        "minecraft:fireball" if !submerged => 14,
        "minecraft:small_fireball" if !submerged => 12,
        "minecraft:dragon_fireball" => 14,
        "minecraft:wither_skull" => 12,
        _ => 0,
    }
}

/// `GuardianEntityLightSource.tick`: the laser's light from its clock (attack
/// time, with no partial tick, and that over the attack duration).
pub fn guardian_laser(time: f32, scale: f32) -> u8 {
    let duration = if scale > 0.0 { time / scale } else { f32::MAX };
    if time >= duration - 5.0 {
        13
    } else {
        (7 + (scale * 5.0) as i32).clamp(0, 15) as u8
    }
}

/// Beacon and end gateway beams: `getLightEmission` of the block (15).
pub const BEAM_LUMINANCE: u8 = 15;

// ---------------------------------------------------------------------------
// Lines and refresh
// ---------------------------------------------------------------------------

/// Points standing for a line light (`LineLightBehavior`, a beacon's beam)
/// near `camera`: every `spacing` blocks along it from `a`, within `reach`
/// of the point of the line nearest the camera, and the line's ends there.
/// They keep their places as the camera moves.
pub fn line_points(
    a: [f64; 3],
    b: [f64; 3],
    camera: [f64; 3],
    spacing: f64,
    reach: f64,
) -> Vec<[f64; 3]> {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    if !len2.is_finite() || len2 < 1e-12 {
        return vec![a];
    }
    let len = len2.sqrt();
    let to_camera = [camera[0] - a[0], camera[1] - a[1], camera[2] - a[2]];
    let along =
        ((to_camera[0] * d[0] + to_camera[1] * d[1] + to_camera[2] * d[2]) / len).clamp(0.0, len);
    let (from, to) = ((along - reach).max(0.0), (along + reach).min(len));
    let at = |s: f64| std::array::from_fn(|i| a[i] + d[i] * (s / len));
    let mut out = Vec::new();
    if from <= 0.0 {
        out.push(a);
    }
    let mut s = (from / spacing).ceil() * spacing;
    if s <= 0.0 {
        s = spacing;
    }
    while s < to && out.len() < 64 {
        out.push(at(s));
        s += spacing;
    }
    if to >= len {
        out.push(b);
    }
    out
}

/// Whether source `id` refreshes this tick (`LambDynLights.shouldTick`):
/// every tick in Fancy, else one tick in the mode's delay, spread by id.
pub fn due(mode: Mode, tick: u64, id: u32) -> bool {
    let delay = u64::from(mode.delay());
    delay <= 1 || tick % delay == u64::from(id) % delay
}

/// `/lambdynamiclights` (or `/lambdynlights`) and what follows it.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Open,
    /// Switches an entity type's light.
    Entity(String, bool),
    /// Lists the entity types switched off.
    Disabled,
    Help,
}

pub fn parse_command(line: &str) -> Option<Command> {
    let mut words = line.split_whitespace();
    let name = words.next()?;
    if name != COMMAND && name != SHORT_COMMAND {
        return None;
    }
    Some(match (words.next(), words.next(), words.next()) {
        (None, _, _) => Command::Open,
        (Some("entity"), Some(id), Some("on")) => Command::Entity(full_id(id), true),
        (Some("entity"), Some(id), Some("off")) => Command::Entity(full_id(id), false),
        (Some("disabled"), _, _) => Command::Disabled,
        _ => Command::Help,
    })
}

pub fn help(ru: bool) -> &'static str {
    if ru {
        "/lambdynamiclights (или /lambdynlights): настройки; entity <id> on|off: свет от сущности (minecraft:zombie...); disabled: у каких сущностей свет выключен"
    } else {
        "/lambdynamiclights (or /lambdynlights): settings; entity <id> on|off: an entity type's light (minecraft:zombie...); disabled: the types switched off"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_originals() {
        let c = Config::default();
        assert_eq!(c.mode, Mode::Fancy);
        assert_eq!(c.creeper, Explosive::Simple);
        assert_eq!(c.tnt, Explosive::Off);
        assert!(c.self_light && c.entities && c.water_sensitive && c.beam);
        assert!(!c.can_light_up("minecraft:glow_item_frame", false));
        assert!(c.can_light_up("minecraft:item_frame", false));
    }

    #[test]
    fn options_round_trip_through_their_files() {
        let mut c = Config::default();
        c.mode = Mode::Fastest;
        c.reach = Reach::Short;
        c.tnt = Explosive::Fancy;
        c.self_light = false;
        c.disabled = vec!["minecraft:blaze".into(), "minecraft:creeper".into()];
        let stored: Vec<(&str, String)> = KEYS.iter().map(|k| (*k, c.value(k))).collect();
        let back = Config::load(|k| {
            stored
                .iter()
                .find(|(kk, _)| *kk == k)
                .map(|(_, v)| v.clone())
        });
        assert_eq!(back, c);
        let odd = Config::load(|k| match k {
            "mode" => Some("FAST".into()),
            "light_sources.creeper" => Some("loud".into()),
            "light_sources.disabled_entities" => Some("zombie, minecraft:skeleton".into()),
            _ => None,
        });
        assert_eq!(odd.mode, Mode::Fast);
        assert_eq!(odd.creeper, Explosive::Simple);
        assert_eq!(odd.disabled, vec!["minecraft:zombie", "minecraft:skeleton"]);
    }

    #[test]
    fn the_page_cycles_and_switches() {
        let mut c = Config::default();
        let p = page();
        assert_eq!(p.len(), 10 + ENTITY_TOGGLES.len() + 1);
        assert_eq!(
            c.label(Setting::Reach, true),
            "Дальность света: Как в оригинале"
        );
        c.click(Setting::Reach, false);
        assert_eq!(c.reach, Reach::Horizon);
        c.click(Setting::Reach, false);
        assert_eq!(c.reach, Reach::Short);
        c.reach = Reach::Original;
        assert_eq!(c.click(Setting::Mode, false), vec!["mode"]);
        assert_eq!(c.mode, Mode::Off);
        c.click(Setting::Mode, true);
        assert_eq!(c.mode, Mode::Fancy);
        c.click(Setting::Tnt, false);
        assert_eq!(c.tnt, Explosive::Simple);
        let glow = ENTITY_TOGGLES
            .iter()
            .position(|e| e.0 == "minecraft:glow_item_frame")
            .unwrap();
        assert_eq!(
            c.label(Setting::Entity(glow), false),
            "Light: Glow Item Frame: OFF"
        );
        c.click(Setting::Entity(glow), false);
        assert!(c.can_light_up("minecraft:glow_item_frame", false));
        assert_eq!(
            c.label(Setting::Entity(glow), true),
            "Свет: Светящаяся рамка: Вкл"
        );
        assert_eq!(
            c.label(Setting::Mode, true),
            "Динамическое освещение: Детальное"
        );
        assert_eq!(c.label(Setting::Reset, false), "Reset to Defaults");
        assert_eq!(c.click(Setting::Reset, false).len(), KEYS.len());
        assert_eq!(c, Config::default());
    }

    #[test]
    fn who_may_light_up() {
        let mut c = Config::default();
        assert!(c.can_light_up("minecraft:player", true));
        c.self_light = false;
        assert!(!c.can_light_up("minecraft:player", true));
        assert!(c.can_light_up("minecraft:player", false));
        c.entities = false;
        assert!(!c.can_light_up("minecraft:zombie", false));
        c.entities = true;
        assert!(c.set_entity("zombie", false));
        assert!(!c.set_entity("minecraft:zombie", false));
        assert!(!c.can_light_up("minecraft:zombie", false));
        assert!(c.set_entity("zombie", true));
        assert!(c.can_light_up("minecraft:zombie", false));
    }

    #[test]
    fn items_follow_the_originals_list_then_their_block() {
        let c = Config::default();
        assert_eq!(item_luminance(&c, "minecraft:torch", false, 14), 14);
        assert_eq!(item_luminance(&c, "minecraft:torch", true, 14), 0);
        assert_eq!(item_luminance(&c, "minecraft:lava_bucket", false, 0), 15);
        assert_eq!(item_luminance(&c, "minecraft:lava_bucket", true, 15), 0);
        assert_eq!(item_luminance(&c, "minecraft:glow_berries", true, 0), 14);
        assert_eq!(item_luminance(&c, "minecraft:blaze_rod", false, 0), 10);
        assert_eq!(item_luminance(&c, "minecraft:lantern", true, 15), 15);
        assert_eq!(item_luminance(&c, "minecraft:stone", false, 0), 0);
        let mut off = c.clone();
        off.water_sensitive = false;
        assert_eq!(item_luminance(&off, "minecraft:soul_torch", true, 10), 10);
    }

    #[test]
    fn living_entities_and_species() {
        let c = Config::default();
        assert_eq!(living(&c, true, false, 3), 15);
        assert_eq!(living(&c, false, true, 3), 15);
        let mut no_glow = c.clone();
        no_glow.glowing_effect = false;
        assert_eq!(living(&no_glow, false, true, 3), 3);
        assert_eq!(mob_luminance(&c, MobLight::Allay), 8);
        assert_eq!(mob_luminance(&c, MobLight::Blaze { wet: false }), 10);
        assert_eq!(mob_luminance(&c, MobLight::Blaze { wet: true }), 4);
        assert_eq!(mob_luminance(&c, MobLight::GlowSquid { dark_ticks: 0 }), 12);
        assert_eq!(mob_luminance(&c, MobLight::GlowSquid { dark_ticks: 5 }), 6);
        assert_eq!(
            mob_luminance(&c, MobLight::GlowSquid { dark_ticks: 100 }),
            0
        );
        assert_eq!(mob_luminance(&c, MobLight::MagmaCube { squish: 0.7 }), 11);
        assert_eq!(mob_luminance(&c, MobLight::MagmaCube { squish: 0.0 }), 8);
        assert_eq!(mob_luminance(&c, MobLight::Creeper { swelling: 0.0 }), 0);
        assert_eq!(mob_luminance(&c, MobLight::Creeper { swelling: 0.5 }), 10);
        let mut fancy = c.clone();
        fancy.creeper = Explosive::Fancy;
        assert_eq!(
            mob_luminance(&fancy, MobLight::Creeper { swelling: 0.55 }),
            5
        );
        fancy.creeper = Explosive::Off;
        assert_eq!(
            mob_luminance(&fancy, MobLight::Creeper { swelling: 0.9 }),
            0
        );
        assert_eq!(mob_luminance(&c, MobLight::Enderman { carried: 15 }), 15);
        assert!((creeper_swelling(14, 30) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn tnt_brightens_as_its_fuse_burns() {
        let mut c = Config::default();
        assert_eq!(tnt_luminance(&c, 40, 80), 0);
        c.tnt = Explosive::Simple;
        assert_eq!(tnt_luminance(&c, 40, 80), 10);
        c.tnt = Explosive::Fancy;
        assert_eq!(tnt_luminance(&c, 80, 80), 0);
        assert_eq!(tnt_luminance(&c, 40, 80), 5);
        assert_eq!(tnt_luminance(&c, 0, 80), 10);
    }

    #[test]
    fn fireballs_go_out_under_water_and_lasers_flare() {
        assert_eq!(projectile_luminance("minecraft:fireball", false), 14);
        assert_eq!(projectile_luminance("minecraft:fireball", true), 0);
        assert_eq!(projectile_luminance("minecraft:small_fireball", false), 12);
        assert_eq!(projectile_luminance("minecraft:wither_skull", true), 12);
        assert_eq!(projectile_luminance("minecraft:snowball", false), 0);
        assert_eq!(guardian_laser(0.0, 0.0), 7);
        assert_eq!(guardian_laser(40.0, 0.5), 9);
        assert_eq!(guardian_laser(76.0, 0.95), 13);
    }

    #[test]
    fn line_points_cover_the_part_near_the_camera_and_stay_put() {
        let a = [0.5, 65.0, 0.5];
        let b = [0.5, 320.0, 0.5];
        let near = line_points(a, b, [5.0, 70.0, 0.0], 3.0, 9.0);
        assert_eq!(near[0], a);
        assert!(near.iter().all(|p| p[1] >= 65.0 && p[1] <= 79.0 + 1e-9));
        assert_eq!(near.len(), 5);
        // Higher up the beam: points on the same 3-block grid from its base.
        let up = line_points(a, b, [5.0, 150.0, 0.0], 3.0, 9.0);
        assert!(
            up.iter()
                .all(|p| ((p[1] - 65.0) / 3.0).fract().abs() < 1e-9)
        );
        assert!(up.iter().all(|p| (p[1] - 150.0).abs() <= 9.0));
        assert_eq!(up.len(), 6);
        // A short laser: all of it, ends included.
        let laser = line_points([0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [2.0, 1.0, 0.0], 3.0, 9.0);
        assert_eq!(
            laser,
            vec![[0.0, 0.0, 0.0], [3.0, 0.0, 0.0], [4.0, 0.0, 0.0]]
        );
        assert_eq!(line_points(a, a, a, 3.0, 9.0), vec![a]);
    }

    #[test]
    fn refresh_is_spread_over_the_mode_delay() {
        assert!((0..10).all(|t| due(Mode::Fancy, t, 7)));
        let ticks: Vec<u64> = (0..20).filter(|t| due(Mode::Fast, *t, 7)).collect();
        assert_eq!(ticks, vec![2, 7, 12, 17]);
        assert_eq!((0..20).filter(|t| due(Mode::Fastest, *t, 3)).count(), 2);
    }

    #[test]
    fn the_command() {
        assert_eq!(parse_command("lambdynlights"), Some(Command::Open));
        assert_eq!(parse_command("lambdynamiclights"), Some(Command::Open));
        assert_eq!(
            parse_command("lambdynlights entity zombie off"),
            Some(Command::Entity("minecraft:zombie".into(), false))
        );
        assert_eq!(
            parse_command("lambdynlights disabled"),
            Some(Command::Disabled)
        );
        assert_eq!(parse_command("lambdynlights what"), Some(Command::Help));
        assert_eq!(parse_command("lambda"), None);
    }

    #[test]
    fn the_shader_gets_the_nearest_sixteen_dimmed_to_the_reach() {
        let camera = [100.0, 64.0, 100.0];
        let mut sources: Vec<([f64; 3], u8)> = (0..40)
            .map(|i| ([100.0 + f64::from(i), 64.0, 100.0], 14))
            .collect();
        sources.push(([100.5, 64.0, 100.0], 0));
        sources.push(([100.0, 64.0, 300.0], 15));
        let out = select(&sources, camera, Reach::Original);
        assert_eq!(out.len(), MAX_LIGHTS);
        assert_eq!(out[0], [0.0, 0.0, 0.0, 14.0 * 0.72]);
        assert_eq!(out[15][0], 15.0);
        assert_eq!(select(&sources, camera, Reach::Horizon)[1][3], 14.0);
        // Equal distances keep their order; NaN and far ones are left out.
        let ties = [
            ([101.0, 64.0, 100.0], 3),
            ([99.0, 64.0, 100.0], 5),
            ([f64::NAN; 3], 15),
        ];
        let out = select(&ties, camera, Reach::Horizon);
        assert_eq!(out.iter().map(|l| l[3]).collect::<Vec<_>>(), vec![3.0, 5.0]);
    }

    #[test]
    fn reach_lowers_living_sources_to_light_the_ground() {
        assert_eq!(Reach::Original.height(1.62), 0.5);
        assert_eq!(Reach::Original.height(0.2), 0.2);
        assert_eq!(Reach::Horizon.height(1.62), 1.62);
        assert!(Reach::Short.scale() < Reach::Original.scale());
    }
}
