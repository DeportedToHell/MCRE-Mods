// Dynamic Lights for Horizon: the mod's settings, texts and light levels,
// with no game types, so the same file builds and tests on its own
// (`cargo test` at the repository root) and is what the bundle adds to the
// game as `crate::game::dynamiclights`.
//
// The lights themselves are Horizon's (`horizon_client::dynamic_light`): the
// game passes the nearest sources to its terrain and entity shaders, which
// raise the block light around each one. This mod chooses the sources.

use std::sync::Mutex;

pub const ID: &str = "dynamiclights";
pub const NAME: &str = "Better Dynamic Lights";
pub const VERSION: &str = "1.0.3";

/// The mod's one key: turns the lights on and off while playing.
pub const TOGGLE_KEY: &str = "toggle";
/// A key that is not bound (the game's `InputConstants.UNKNOWN`).
pub const UNBOUND: &str = "key.keyboard.unknown";

/// What Horizon shaders accept: a light level is at most 15.
pub const MAX_LEVEL: u8 = 15;
/// What the shaders hold at once.
pub const MAX_SOURCES: u8 = 16;

pub fn russian(language: &str) -> bool {
    language.starts_with("ru")
}

/// One text in both languages.
fn t(ru: bool, en: &'static str, russian: &'static str) -> &'static str {
    if ru { russian } else { en }
}

pub fn description(ru: bool) -> &'static str {
    t(
        ru,
        "Horizon's built-in dynamic lights, with more: besides held and dropped items and burning mobs, glowing \
         creatures, projectiles, TNT and minecarts shine too, water puts out fire, and every kind of light has its \
         own switch.",
        "Встроенный динамический свет Horizon, только больше: кроме предметов в руке, на земле и горящих мобов \
         светятся ещё светящиеся существа, снаряды, динамит и вагонетки, вода гасит огонь, и у каждого вида света \
         свой переключатель.",
    )
}

// ---- light sources ---------------------------------------------------------

/// The kinds of light sources, each with its own switch.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Glowing items in the player's own hands.
    OwnHands,
    /// Glowing items held by other players.
    Players,
    /// Glowing items held by mobs and armor stands.
    Mobs,
    /// Glowing items lying on the ground.
    Dropped,
    /// Anything on fire.
    Burning,
    /// Creatures that glow by themselves.
    Creatures,
    /// Fireballs, skulls, bullets, spectral arrows, rockets.
    Projectiles,
    /// Primed TNT, lit TNT minecarts, running furnace minecarts.
    Explosives,
}

impl Kind {
    pub const ALL: [Kind; 8] = [
        Kind::OwnHands,
        Kind::Players,
        Kind::Mobs,
        Kind::Dropped,
        Kind::Burning,
        Kind::Creatures,
        Kind::Projectiles,
        Kind::Explosives,
    ];

    fn index(self) -> usize {
        Kind::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }

    /// Its setting's name (a file in the mod's data folder).
    fn key(self) -> &'static str {
        match self {
            Kind::OwnHands => "own_hands",
            Kind::Players => "players",
            Kind::Mobs => "mobs",
            Kind::Dropped => "dropped_items",
            Kind::Burning => "burning",
            Kind::Creatures => "creatures",
            Kind::Projectiles => "projectiles",
            Kind::Explosives => "explosives",
        }
    }
}

/// How brightly an entity on fire shines: as fire does (Horizon's `BURNING`).
pub const BURNING: u8 = 15;

/// Extra light for items whose block has another name: glow berries are
/// what makes cave vines shine (14).
pub fn item_extra(item: &str) -> u8 {
    match item {
        "minecraft:glow_berries" => 14,
        _ => 0,
    }
}

/// Items whose light is a flame or lava, which water puts out: torches
/// (not a redstone torch), campfires and a lava bucket. Lanterns keep their
/// flame behind glass.
pub fn fire_item(item: &str) -> bool {
    let name = item.rsplit(':').next().unwrap_or(item);
    (name.ends_with("torch") && !name.contains("redstone")) || name.ends_with("campfire") || name == "lava_bucket"
}

/// The light of a creature that glows by itself (vanilla draws these at
/// full brightness), 0 for the rest. `size` is a magma cube's size, and
/// `dark_ticks` how long a hurt glow squid stays dark.
pub fn creature_light(kind: &str, size: i32, dark_ticks: i32) -> u8 {
    match kind {
        "minecraft:blaze" => 10,
        "minecraft:magma_cube" => match size {
            ..=1 => 6,
            2..=3 => 8,
            _ => 10,
        },
        // As `GlowSquidRenderer` fades it: dark while hurt, back over the
        // last 10 dark ticks.
        "minecraft:glow_squid" => {
            let glow = (1.0 - dark_ticks as f32 / 10.0).clamp(0.0, 1.0);
            (8.0 * glow).round() as u8
        }
        "minecraft:allay" | "minecraft:vex" => 6,
        _ => 0,
    }
}

/// Creatures made of fire, which water puts out.
pub fn fire_creature(kind: &str) -> bool {
    kind == "minecraft:blaze"
}

/// The light of each projectile that glows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Glow {
    LargeFireball,
    SmallFireball,
    DragonFireball,
    WitherSkull,
    ShulkerBullet,
    SpectralArrow,
    Rocket,
    PrimedTnt,
    TntMinecart,
    FurnaceMinecart,
}

impl Glow {
    pub fn level(self) -> u8 {
        match self {
            Glow::LargeFireball => 15,
            Glow::SmallFireball => 12,
            Glow::DragonFireball => 10,
            Glow::WitherSkull => 6,
            Glow::ShulkerBullet | Glow::SpectralArrow | Glow::Rocket => 8,
            Glow::PrimedTnt | Glow::TntMinecart => 10,
            // A lit furnace.
            Glow::FurnaceMinecart => 13,
        }
    }

    /// Flames, which water puts out.
    pub fn fire(self) -> bool {
        matches!(self, Glow::LargeFireball | Glow::SmallFireball)
    }

    pub fn kind(self) -> Kind {
        match self {
            Glow::PrimedTnt | Glow::TntMinecart | Glow::FurnaceMinecart => Kind::Explosives,
            _ => Kind::Projectiles,
        }
    }
}

// ---- settings --------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    pub enabled: bool,
    pub kinds: [bool; 8],
    /// Flames and lava give no light underwater.
    pub water: bool,
    /// Percent of the full level, 25 to 100.
    pub brightness: u8,
    /// How many of the nearest sources light up, 1 to 16.
    pub max_sources: u8,
    /// Blocks from the camera beyond which sources give no light.
    pub range: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: true,
            kinds: [true; 8],
            water: true,
            brightness: 100,
            max_sources: MAX_SOURCES,
            range: 64,
        }
    }
}

/// A slider's numbers: smallest, largest and the step between.
#[derive(Clone, Copy, Debug)]
struct Steps {
    min: u8,
    max: u8,
    step: u8,
}

impl Steps {
    fn clamp(self, v: u8) -> u8 {
        let v = v.clamp(self.min, self.max);
        self.min + (v - self.min) / self.step * self.step
    }
    /// Where `v` sits on the slider, 0 to 1.
    fn fraction(self, v: u8) -> f32 {
        f32::from(v.saturating_sub(self.min)) / f32::from(self.max - self.min)
    }
    /// The step nearest `f` (0 to 1) along the slider.
    fn at(self, f: f32) -> u8 {
        let steps = f32::from((self.max - self.min) / self.step);
        let n = (f.clamp(0.0, 1.0) * steps).round() as u8;
        self.min + n * self.step
    }
}

const BRIGHTNESS: Steps = Steps { min: 25, max: 100, step: 5 };
const SOURCES: Steps = Steps { min: 1, max: MAX_SOURCES, step: 1 };
const RANGE: Steps = Steps { min: 16, max: 96, step: 8 };

/// One option of the settings screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Opt {
    Enabled,
    Kind(Kind),
    Water,
    Brightness,
    MaxSources,
    Range,
}

impl Opt {
    fn steps(self) -> Option<Steps> {
        match self {
            Opt::Brightness => Some(BRIGHTNESS),
            Opt::MaxSources => Some(SOURCES),
            Opt::Range => Some(RANGE),
            _ => None,
        }
    }

    /// A slider rather than an ON/OFF button.
    pub fn slider(self) -> bool {
        self.steps().is_some()
    }

    pub fn caption(self, ru: bool) -> &'static str {
        match self {
            Opt::Enabled => t(ru, "Dynamic Lights", "Динамический свет"),
            Opt::Kind(Kind::OwnHands) => t(ru, "Your Hands", "Ваши руки"),
            Opt::Kind(Kind::Players) => t(ru, "Other Players", "Другие игроки"),
            Opt::Kind(Kind::Mobs) => t(ru, "Mobs", "Мобы"),
            Opt::Kind(Kind::Dropped) => t(ru, "Dropped Items", "Предметы на земле"),
            Opt::Kind(Kind::Burning) => t(ru, "Burning Entities", "Горящие сущности"),
            Opt::Kind(Kind::Creatures) => t(ru, "Glowing Creatures", "Светящиеся существа"),
            Opt::Kind(Kind::Projectiles) => t(ru, "Projectiles", "Снаряды"),
            Opt::Kind(Kind::Explosives) => t(ru, "TNT and Minecarts", "Динамит и вагонетки"),
            Opt::Water => t(ru, "Water Puts Out Fire", "Вода гасит огонь"),
            Opt::Brightness => t(ru, "Brightness", "Яркость"),
            Opt::MaxSources => t(ru, "Max Light Sources", "Макс. источников"),
            Opt::Range => t(ru, "Range", "Дальность"),
        }
    }

    pub fn tooltip(self, ru: bool) -> &'static str {
        match self {
            Opt::Enabled => t(
                ru,
                "Glowing items, burning entities and glowing creatures light up the blocks and mobs around them \
                 as they move. While on, this takes the place of Dynamic lights in Horizon Extras.",
                "Светящиеся предметы, горящие сущности и светящиеся существа освещают блоки и мобов вокруг себя \
                 на ходу. Пока включено, заменяет динамический свет из «Дополнений Horizon».",
            ),
            Opt::Kind(Kind::OwnHands) => t(
                ru,
                "A torch, lantern or other glowing item in your main hand or off hand lights your way.",
                "Факел, фонарь или другой светящийся предмет в основной или второй руке освещает вам путь.",
            ),
            Opt::Kind(Kind::Players) => t(
                ru,
                "Glowing items held by other players.",
                "Светящиеся предметы в руках других игроков.",
            ),
            Opt::Kind(Kind::Mobs) => t(
                ru,
                "Glowing items held by mobs and armor stands, such as a zombie with a torch.",
                "Светящиеся предметы в руках мобов и стоек для брони, например зомби с факелом.",
            ),
            Opt::Kind(Kind::Dropped) => t(
                ru,
                "Glowing items lying on the ground.",
                "Светящиеся предметы, лежащие на земле.",
            ),
            Opt::Kind(Kind::Burning) => t(
                ru,
                "Players, mobs, items and arrows on fire shine as brightly as fire.",
                "Игроки, мобы, предметы и стрелы в огне светят ярко, как сам огонь.",
            ),
            Opt::Kind(Kind::Creatures) => t(
                ru,
                "Blazes, magma cubes, glow squids, allays and vexes give off light of their own.",
                "Всполохи, магмовые кубы, светящиеся спруты, тихони и вредины светятся сами.",
            ),
            Opt::Kind(Kind::Projectiles) => t(
                ru,
                "Fireballs, dragon fireballs, wither skulls, shulker bullets, spectral arrows and firework rockets \
                 light their way.",
                "Файерболы, файерболы дракона, черепа визера, снаряды шалкера, спектральные стрелы и фейерверки \
                 освещают свой путь.",
            ),
            Opt::Kind(Kind::Explosives) => t(
                ru,
                "Primed TNT, a lit minecart with TNT and a running minecart with furnace glow.",
                "Светятся активированный динамит, подожжённая вагонетка с динамитом и едущая самоходная вагонетка.",
            ),
            Opt::Water => t(
                ru,
                "Torches, campfires, lava buckets, fireballs and blazes give no light underwater.",
                "Факелы, костры, вёдра лавы, файерболы и всполохи не светят под водой.",
            ),
            Opt::Brightness => t(
                ru,
                "How bright the lights are, compared with the light the same blocks give when placed.",
                "Насколько ярок свет по сравнению со светом тех же блоков, если их поставить.",
            ),
            Opt::MaxSources => t(
                ru,
                "How many of the light sources nearest to you shine at once (16 at most). Fewer is faster on a \
                 weak graphics card.",
                "Сколько ближайших источников света светят одновременно (не больше 16). Чем меньше, тем быстрее \
                 на слабой видеокарте.",
            ),
            Opt::Range => t(
                ru,
                "Light sources farther than this from the camera give no light.",
                "Источники света дальше этого расстояния от камеры не светят.",
            ),
        }
    }
}

/// "64 Blocks", "64 блока".
pub fn blocks(n: u8, ru: bool) -> String {
    if !ru {
        return format!("{n} Blocks");
    }
    let word = match (n % 10, n % 100) {
        (_, 11..=14) => "блоков",
        (1, _) => "блок",
        (2..=4, _) => "блока",
        _ => "блоков",
    };
    format!("{n} {word}")
}

impl Settings {
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Settings {
        let d = Settings::default();
        let flag = |k: &str, d: bool| match get(k).as_deref().map(str::trim) {
            Some("true") => true,
            Some("false") => false,
            _ => d,
        };
        let number = |k: &str, steps: Steps, d: u8| {
            get(k).and_then(|v| v.trim().parse::<u8>().ok()).map_or(d, |v| steps.clamp(v))
        };
        let mut kinds = d.kinds;
        for k in Kind::ALL {
            kinds[k.index()] = flag(k.key(), d.kinds[k.index()]);
        }
        Settings {
            enabled: flag("enabled", d.enabled),
            kinds,
            water: flag("water_puts_out_fire", d.water),
            brightness: number("brightness", BRIGHTNESS, d.brightness),
            max_sources: number("max_sources", SOURCES, d.max_sources),
            range: number("range", RANGE, d.range),
        }
    }

    /// Every setting as it is kept: `(name, value)`.
    pub fn stored(&self) -> Vec<(&'static str, String)> {
        let mut out = vec![("enabled", self.enabled.to_string())];
        out.extend(Kind::ALL.iter().map(|k| (k.key(), self.on(*k).to_string())));
        out.push(("water_puts_out_fire", self.water.to_string()));
        out.push(("brightness", self.brightness.to_string()));
        out.push(("max_sources", self.max_sources.to_string()));
        out.push(("range", self.range.to_string()));
        out
    }

    pub fn on(&self, kind: Kind) -> bool {
        self.kinds[kind.index()]
    }

    /// A switch's state; None for a slider.
    pub fn switch(&self, opt: Opt) -> Option<bool> {
        match opt {
            Opt::Enabled => Some(self.enabled),
            Opt::Kind(k) => Some(self.on(k)),
            Opt::Water => Some(self.water),
            _ => None,
        }
    }

    /// A switch pressed: it flips.
    pub fn flip(&mut self, opt: Opt) {
        match opt {
            Opt::Enabled => self.enabled = !self.enabled,
            Opt::Kind(k) => self.kinds[k.index()] = !self.kinds[k.index()],
            Opt::Water => self.water = !self.water,
            _ => {}
        }
    }

    fn number(&mut self, opt: Opt) -> Option<&mut u8> {
        match opt {
            Opt::Brightness => Some(&mut self.brightness),
            Opt::MaxSources => Some(&mut self.max_sources),
            Opt::Range => Some(&mut self.range),
            _ => None,
        }
    }

    /// Where a slider's handle is, 0 to 1.
    pub fn fraction(&self, opt: Opt) -> f32 {
        let mut s = *self;
        match (opt.steps(), s.number(opt)) {
            (Some(steps), Some(v)) => steps.fraction(*v),
            _ => 0.0,
        }
    }

    /// A slider moved to `f` (0 to 1): true when its value changed.
    pub fn set_fraction(&mut self, opt: Opt, f: f32) -> bool {
        let Some(steps) = opt.steps() else { return false };
        let Some(v) = self.number(opt) else { return false };
        let new = steps.at(f);
        std::mem::replace(v, new) != new
    }

    /// A slider stepped once with the keyboard: up or down.
    pub fn step(&mut self, opt: Opt, up: bool) -> bool {
        let Some(steps) = opt.steps() else { return false };
        let Some(v) = self.number(opt) else { return false };
        let new = if up { v.saturating_add(steps.step) } else { v.saturating_sub(steps.step) };
        let new = steps.clamp(new.max(steps.min));
        std::mem::replace(v, new) != new
    }

    /// A slider's value as it shows: "100%", "16", "64 Blocks".
    pub fn number_text(&self, opt: Opt, ru: bool) -> String {
        match opt {
            Opt::Brightness => format!("{}%", self.brightness),
            Opt::MaxSources => self.max_sources.to_string(),
            Opt::Range => blocks(self.range, ru),
            _ => String::new(),
        }
    }

    /// A light level as the shaders get it, with the brightness applied.
    pub fn shown(&self, level: f32) -> f32 {
        level.min(f32::from(MAX_LEVEL)) * f32::from(self.brightness) / 100.0
    }

    /// The Horizon page's buttons: Configure... (Mod Menu's `modmenu.configure`)
    /// and the main switch.
    pub fn page_labels(&self, ru: bool) -> Vec<String> {
        let state = match (self.enabled, ru) {
            (true, false) => "ON",
            (false, false) => "OFF",
            (true, true) => "Вкл",
            (false, true) => "Выкл",
        };
        vec![
            t(ru, "Configure...", "Настройки...").to_owned(),
            format!("{}: {state}", Opt::Enabled.caption(ru)),
        ]
    }
}

/// The screen's title.
pub fn title(_ru: bool) -> &'static str {
    NAME
}

/// The headings over the screen's groups of options.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Heading {
    Sources,
    Lighting,
    Controls,
}

impl Heading {
    pub fn text(self, ru: bool) -> &'static str {
        match self {
            Heading::Sources => t(ru, "Light Sources", "Источники света"),
            Heading::Lighting => t(ru, "Lighting", "Освещение"),
            Heading::Controls => t(ru, "Controls", "Управление"),
        }
    }
}

/// The toggle key's name in the controls row and on the Horizon page.
pub fn key_label(ru: bool) -> &'static str {
    t(ru, "Toggle Dynamic Lights", "Вкл./выкл. динамический свет")
}

static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);

/// The settings, read once from the game's store.
pub fn settings(get: impl Fn(&str) -> Option<String>) -> Settings {
    let mut kept = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    *kept.get_or_insert_with(|| Settings::read(get))
}

pub fn set_settings(settings: Settings) {
    *SETTINGS.lock().unwrap_or_else(|e| e.into_inner()) = Some(settings);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn settings_round_trip() {
        let mut s = Settings::default();
        s.enabled = false;
        s.flip(Opt::Kind(Kind::Dropped));
        s.water = false;
        s.brightness = 60;
        s.max_sources = 4;
        s.range = 32;
        let kept: HashMap<_, _> = s.stored().into_iter().collect();
        assert_eq!(kept.len(), 13);
        let back = Settings::read(|k| kept.get(k).cloned());
        assert_eq!(back, s);
    }

    #[test]
    fn missing_and_bad_values_are_defaults() {
        let s = Settings::read(|k| match k {
            "enabled" => Some("maybe".into()),
            "brightness" => Some("7".into()),
            "range" => Some("250".into()),
            "max_sources" => Some(" 9 \n".into()),
            _ => None,
        });
        assert!(s.enabled);
        assert_eq!(s.brightness, 25);
        assert_eq!(s.range, 96);
        assert_eq!(s.max_sources, 9);
        assert_eq!(Settings::read(|_| None), Settings::default());
    }

    #[test]
    fn sliders_move_in_steps() {
        let mut s = Settings::default();
        assert_eq!(s.fraction(Opt::Brightness), 1.0);
        assert!(s.set_fraction(Opt::Brightness, 0.0));
        assert_eq!(s.brightness, 25);
        assert!(!s.set_fraction(Opt::Brightness, 0.01));
        assert!(s.set_fraction(Opt::Range, 0.5));
        assert_eq!(s.range, 56);
        assert!(s.step(Opt::Range, true));
        assert_eq!(s.range, 64);
        s.range = 96;
        assert!(!s.step(Opt::Range, true));
        s.max_sources = 1;
        assert!(!s.step(Opt::MaxSources, false));
        assert!(s.step(Opt::MaxSources, true));
        assert_eq!(s.max_sources, 2);
        assert!(!s.set_fraction(Opt::Water, 0.5));
    }

    #[test]
    fn fire_goes_out_but_lanterns_stay() {
        for item in ["minecraft:torch", "minecraft:soul_torch", "minecraft:copper_torch", "minecraft:campfire",
            "minecraft:soul_campfire", "minecraft:lava_bucket"]
        {
            assert!(fire_item(item), "{item}");
        }
        for item in ["minecraft:redstone_torch", "minecraft:lantern", "minecraft:glowstone", "minecraft:sea_lantern"] {
            assert!(!fire_item(item), "{item}");
        }
    }

    #[test]
    fn creatures_glow() {
        assert_eq!(creature_light("minecraft:blaze", 0, 0), 10);
        assert_eq!(creature_light("minecraft:magma_cube", 1, 0), 6);
        assert_eq!(creature_light("minecraft:magma_cube", 4, 0), 10);
        assert_eq!(creature_light("minecraft:glow_squid", 0, 0), 8);
        assert_eq!(creature_light("minecraft:glow_squid", 0, 100), 0);
        assert_eq!(creature_light("minecraft:glow_squid", 0, 5), 4);
        assert_eq!(creature_light("minecraft:zombie", 0, 0), 0);
        assert!(fire_creature("minecraft:blaze") && !fire_creature("minecraft:magma_cube"));
    }

    #[test]
    fn russian_block_counts() {
        assert_eq!(blocks(16, true), "16 блоков");
        assert_eq!(blocks(24, true), "24 блока");
        assert_eq!(blocks(64, true), "64 блока");
        assert_eq!(blocks(72, true), "72 блока");
        assert_eq!(blocks(96, false), "96 Blocks");
        assert_eq!(blocks(21, true), "21 блок");
        assert_eq!(blocks(12, true), "12 блоков");
    }

    #[test]
    fn brightness_scales_levels() {
        let mut s = Settings::default();
        assert_eq!(s.shown(14.0), 14.0);
        assert_eq!(s.shown(20.0), 15.0);
        s.brightness = 50;
        assert_eq!(s.shown(14.0), 7.0);
    }

    #[test]
    fn page_labels() {
        let s = Settings::default();
        assert_eq!(s.page_labels(false), ["Configure...", "Dynamic Lights: ON"]);
        assert_eq!(s.page_labels(true), ["Настройки...", "Динамический свет: Вкл"]);
    }
}
