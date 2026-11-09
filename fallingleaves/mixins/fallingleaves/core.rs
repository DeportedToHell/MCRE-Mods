// Falling Leaves for Horizon: its settings (`FallingLeavesConfig`, the same
// `fallingleaves2.json`), each leaf block's settings and defaults
// (`LeafSettingsEntry`, `ConfigDefaults`), the wind (`Wind`), and where leaves
// and snowflakes start (`LeafUtil`, `LeafTickMixin`, `LeafTickMixin2`,
// `BlockMixin`). Rewritten from Falling Leaves by RandomMcSomethin (MIT,
// https://github.com/RandomMcSomethin/fallingleaves).
//
// Spliced into the game's ambient effects (`horizon_client::animate_tick`), so
// the hook on `block_animate_tick` reaches it and it can read the module's own
// tables. The game side (`fallingleaves_glue`) makes the particles it queues.
use super::*;
use crate::particles::fallingleaves_particle::Kind;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Mutex, RwLock};

pub const ID: &str = "fallingleaves";
pub const NAME: &str = "Falling Leaves";
pub const VERSION: &str = "1.0.0";
/// `ConfigDefaults.DEFAULT_SPAWN_CHANCE`: vanilla's 1%.
pub const DEFAULT_SPAWN_CHANCE: f32 = 0.01;
pub const CONFIG_FILE: &str = "fallingleaves2.json";

// ---- the settings -----------------------------------------------------------------------

/// `ParticleImplementation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Implementation {
    Vanilla,
    Regular,
    Conifer,
    Cherry,
}

impl Implementation {
    pub const ALL: [Implementation; 4] = [Self::Vanilla, Self::Regular, Self::Conifer, Self::Cherry];
    pub fn name(self) -> &'static str {
        match self {
            Self::Vanilla => "VANILLA",
            Self::Regular => "REGULAR",
            Self::Conifer => "CONIFER",
            Self::Cherry => "CHERRY",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|i| i.name() == s)
    }
}

/// What kind of block a leaf block is in the game, which its defaults follow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LeafClass {
    /// `TintedParticleLeavesBlock` itself, with its `leafParticleChance`.
    Tinted(f32),
    /// `UntintedParticleLeavesBlock` (cherry, pale oak, poplars), or a tinted one
    /// with its own particle.
    Untinted(f32),
    /// Another `LeavesBlock` (a content mod's).
    OtherLeaves,
    /// Not leaves: a block from "Additional block states that drop leaves".
    NotLeaves,
}

/// `LeafSettingsEntry`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeafSettings {
    pub implementation: Implementation,
    pub spawn_rate_factor: f32,
    pub is_conifer: bool,
    pub spawn_breaking_leaves: bool,
}

impl LeafSettings {
    /// `new LeafSettingsEntry(id)`: the defaults.
    pub fn defaults(id: &str, class: LeafClass) -> LeafSettings {
        LeafSettings {
            implementation: default_implementation(id, class),
            spawn_rate_factor: default_spawn_rate_factor(id, class),
            is_conifer: is_conifer(id),
            spawn_breaking_leaves: true,
        }
    }
    /// `getImplementation`.
    pub fn implementation(&self, cfg: &Config) -> Implementation {
        if cfg.always_use_vanilla_particles { Implementation::Vanilla } else { self.implementation }
    }
    /// `getSpawnChance`.
    pub fn spawn_chance(&self, cfg: &Config) -> f32 {
        let base = if self.is_conifer { cfg.base_conifer_leaf_spawn_chance() } else { cfg.base_leaf_spawn_chance() };
        self.spawn_rate_factor * base
    }
}

/// `ConfigDefaults.getImplementation`.
pub fn default_implementation(id: &str, class: LeafClass) -> Implementation {
    if is_conifer(id) {
        Implementation::Conifer
    } else if matches!(class, LeafClass::Untinted(_) | LeafClass::OtherLeaves) {
        Implementation::Vanilla
    } else {
        Implementation::Regular
    }
}

/// `ConfigDefaults.isConifer`.
pub fn is_conifer(id: &str) -> bool {
    matches!(
        id,
        "bewitchment:cypress_leaves"
            | "bewitchment:dragons_blood_leaves"
            | "bewitchment:juniper_leaves"
            | "biomemakeover:swamp_cypress_leaves"
            | "byg:araucaria_leaves"
            | "byg:blue_spruce_leaves"
            | "byg:cypress_leaves"
            | "byg:fir_leaves"
            | "byg:orange_spruce_leaves"
            | "byg:pine_leaves"
            | "byg:red_spruce_leaves"
            | "byg:redwood_leaves"
            | "byg:yellow_spruce_leaves"
            | "minecraft:spruce_leaves"
            | "terrestria:cypress_leaves"
            | "terrestria:hemlock_leaves"
            | "terrestria:redwood_leaves"
            | "traverse:fir_leaves"
            | "woods_and_mires:pine_leaves"
    )
}

/// `ConfigDefaults.spawnRateFactor`: none for shrubs and large-leaved trees,
/// otherwise the block's own leaf particle chance against vanilla's 1%.
pub fn default_spawn_rate_factor(id: &str, class: LeafClass) -> f32 {
    if matches!(
        id,
        "byg:palm_leaves"
            | "minecraft:jungle_leaves"
            | "promenade:palm_leaves"
            | "terrestria:japanese_maple_shrub_leaves"
            | "terrestria:jungle_palm_leaves"
            | "terrestria:yucca_palm_leaves"
    ) {
        return 0.0;
    }
    match class {
        LeafClass::Tinted(chance) | LeafClass::Untinted(chance) => chance / DEFAULT_SPAWN_CHANCE,
        _ => 1.0,
    }
}

/// `FallingLeavesConfig`, as `fallingleaves2.json` keeps it.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub display_debug_data: bool,
    pub enabled: bool,
    pub always_use_vanilla_particles: bool,
    pub use_vanilla_textures: bool,
    pub leaf_size: i32,
    pub leaf_lifespan: i32,
    pub leaf_spawn_rate: i32,
    pub conifer_leaf_spawn_rate: i32,
    pub snowflake_spawn_rate: i32,
    pub drop_from_player_placed_blocks: bool,
    pub leaves_on_block_hit: bool,
    pub minimum_free_space_below: i32,
    pub wind_enabled: bool,
    pub windless_dimensions: Vec<String>,
    /// Only the entries that differ from their block's defaults, and the
    /// spawners' (`validatePostLoad`).
    pub leaf_settings: BTreeMap<String, LeafSettings>,
    pub leaf_spawners: Vec<String>,
    pub fall_spawn_rate_factor: f32,
    pub winter_spawn_rate_factor: f32,
    pub starting_spawn_radius: i32,
    pub decay_spawn_rate_factor: f32,
    pub max_decay_leaves: i32,
    pub register_particles: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            display_debug_data: false,
            enabled: true,
            always_use_vanilla_particles: false,
            use_vanilla_textures: false,
            leaf_size: 5,
            leaf_lifespan: 200,
            leaf_spawn_rate: 20,
            conifer_leaf_spawn_rate: 0,
            snowflake_spawn_rate: 30,
            drop_from_player_placed_blocks: true,
            leaves_on_block_hit: true,
            minimum_free_space_below: 1,
            wind_enabled: true,
            windless_dimensions: vec!["minecraft:the_nether".into(), "minecraft:the_end".into()],
            leaf_settings: BTreeMap::new(),
            leaf_spawners: Vec::new(),
            fall_spawn_rate_factor: 1.8,
            winter_spawn_rate_factor: 0.1,
            starting_spawn_radius: 0,
            decay_spawn_rate_factor: 2.6,
            max_decay_leaves: 9,
            register_particles: true,
        }
    }
}

/// A block id with its namespace (`oak_leaves` -> `minecraft:oak_leaves`).
pub fn full_id(name: &str) -> String {
    if name.contains(':') { name.to_owned() } else { format!("minecraft:{name}") }
}

impl Config {
    /// `getLeafSize`.
    pub fn leaf_size(&self) -> f32 {
        self.leaf_size as f32 / 50.0
    }
    pub fn base_leaf_spawn_chance(&self) -> f32 {
        self.leaf_spawn_rate as f32 / 20.0 * DEFAULT_SPAWN_CHANCE
    }
    pub fn base_conifer_leaf_spawn_chance(&self) -> f32 {
        self.conifer_leaf_spawn_rate as f32 / 20.0 * DEFAULT_SPAWN_CHANCE
    }
    pub fn snowflake_spawn_chance(&self) -> f32 {
        self.snowflake_spawn_rate as f32 / 20.0 * DEFAULT_SPAWN_CHANCE
    }

    /// `leafSettings.get(id)`: the block's entry, its defaults for a leaf block
    /// or a spawner, none for any other block.
    pub fn leaf_settings(&self, id: &str, class: LeafClass) -> Option<LeafSettings> {
        if let Some(s) = self.leaf_settings.get(id) {
            return Some(*s);
        }
        let spawner = self.is_leaf_spawner(id);
        (class != LeafClass::NotLeaves || spawner).then(|| LeafSettings::defaults(id, class))
    }

    /// The spawners' ids (`leafSpawnerIds`): what comes before `[`.
    pub fn spawner_ids(&self) -> impl Iterator<Item = String> + '_ {
        self.leaf_spawners.iter().map(|s| full_id(s.split('[').next().unwrap_or(s).trim()))
    }
    pub fn is_leaf_spawner(&self, id: &str) -> bool {
        self.spawner_ids().any(|s| s == id)
    }
    /// `getLeafSpawnerProperties`: the `[key=value,...]` a spawner asks for.
    pub fn spawner_properties(&self, id: &str) -> Vec<(String, String)> {
        for s in &self.leaf_spawners {
            let (head, rest) = s.split_once('[').unwrap_or((s, ""));
            if full_id(head.trim()) != id {
                continue;
            }
            return rest
                .trim_end_matches(']')
                .split(',')
                .filter_map(|kv| {
                    let (k, v) = kv.split_once('=')?;
                    Some((k.trim().to_owned(), v.trim().to_owned()))
                })
                .collect();
        }
        Vec::new()
    }

    /// `validatePostLoad`.
    pub fn validate(&mut self) {
        self.leaf_size = self.leaf_size.max(1);
        self.minimum_free_space_below = self.minimum_free_space_below.max(1);
    }

    // ---- the file ----

    /// Gson's reading of `fallingleaves2.json`: a missing or wrong field keeps
    /// its default.
    pub fn parse(text: &str) -> Config {
        let mut c = Config::default();
        let Ok(Value::Object(m)) = serde_json::from_str::<Value>(text) else {
            return c;
        };
        let b = |k: &str, d: bool| m.get(k).and_then(Value::as_bool).unwrap_or(d);
        let i = |k: &str, d: i32| m.get(k).and_then(Value::as_i64).map_or(d, |v| v as i32);
        let f = |k: &str, d: f32| m.get(k).and_then(Value::as_f64).map_or(d, |v| v as f32);
        let list = |k: &str, d: &Vec<String>| {
            m.get(k).and_then(Value::as_array).map_or_else(
                || d.clone(),
                |a| a.iter().filter_map(Value::as_str).map(str::to_owned).collect(),
            )
        };
        c.display_debug_data = b("displayDebugData", c.display_debug_data);
        c.enabled = b("enabled", c.enabled);
        c.always_use_vanilla_particles = b("alwaysUseVanillaParticles", c.always_use_vanilla_particles);
        c.use_vanilla_textures = b("useVanillaTextures", c.use_vanilla_textures);
        c.leaf_size = i("leafSize", c.leaf_size);
        c.leaf_lifespan = i("leafLifespan", c.leaf_lifespan);
        c.leaf_spawn_rate = i("leafSpawnRate", c.leaf_spawn_rate);
        c.conifer_leaf_spawn_rate = i("coniferLeafSpawnRate", c.conifer_leaf_spawn_rate);
        c.snowflake_spawn_rate = i("snowflakeSpawnRate", c.snowflake_spawn_rate);
        c.drop_from_player_placed_blocks = b("dropFromPlayerPlacedBlocks", c.drop_from_player_placed_blocks);
        c.leaves_on_block_hit = b("leavesOnBlockHit", c.leaves_on_block_hit);
        c.minimum_free_space_below = i("minimumFreeSpaceBelow", c.minimum_free_space_below);
        c.wind_enabled = b("windEnabled", c.wind_enabled);
        c.windless_dimensions = list("windlessDimensions", &c.windless_dimensions);
        c.leaf_spawners = list("leafSpawners", &c.leaf_spawners);
        c.fall_spawn_rate_factor = f("fallSpawnRateFactor", c.fall_spawn_rate_factor);
        c.winter_spawn_rate_factor = f("winterSpawnRateFactor", c.winter_spawn_rate_factor);
        c.starting_spawn_radius = i("startingSpawnRadius", c.starting_spawn_radius);
        c.decay_spawn_rate_factor = f("decaySpawnRateFactor", c.decay_spawn_rate_factor);
        c.max_decay_leaves = i("maxDecayLeaves", c.max_decay_leaves);
        c.register_particles = b("registerParticles", c.register_particles);
        if let Some(Value::Object(entries)) = m.get("leafSettings") {
            for (id, e) in entries {
                let Value::Object(e) = e else { continue };
                // Gson fills what the entry leaves out with the class's own
                // defaults, which a missing field can't tell here: take REGULAR,
                // 1, false and true as a field's fallback.
                let s = LeafSettings {
                    implementation: e
                        .get("particleImplementation")
                        .and_then(Value::as_str)
                        .and_then(Implementation::parse)
                        .unwrap_or(Implementation::Regular),
                    spawn_rate_factor: e.get("spawnRateFactor").and_then(Value::as_f64).map_or(1.0, |v| v as f32),
                    is_conifer: e.get("isConiferBlock").and_then(Value::as_bool).unwrap_or(false),
                    spawn_breaking_leaves: e.get("spawnBreakingLeaves").and_then(Value::as_bool).unwrap_or(true),
                };
                c.leaf_settings.insert(full_id(id), s);
            }
        }
        c.validate();
        c
    }

    /// The file as Gson writes it (pretty, the fields in the class's order),
    /// leaving out the leaf entries that match their block's defaults.
    pub fn to_json(&self, class: &dyn Fn(&str) -> LeafClass) -> String {
        let mut o = String::from("{\n");
        let mut field = |name: &str, value: String| {
            let _ = writeln!(o, "  \"{name}\": {value},");
        };
        field("version", "2".into());
        field("displayDebugData", self.display_debug_data.to_string());
        field("enabled", self.enabled.to_string());
        field("alwaysUseVanillaParticles", self.always_use_vanilla_particles.to_string());
        field("useVanillaTextures", self.use_vanilla_textures.to_string());
        field("leafSize", self.leaf_size.to_string());
        field("leafLifespan", self.leaf_lifespan.to_string());
        field("leafSpawnRate", self.leaf_spawn_rate.to_string());
        field("coniferLeafSpawnRate", self.conifer_leaf_spawn_rate.to_string());
        field("snowflakeSpawnRate", self.snowflake_spawn_rate.to_string());
        field("dropFromPlayerPlacedBlocks", self.drop_from_player_placed_blocks.to_string());
        field("leavesOnBlockHit", self.leaves_on_block_hit.to_string());
        field("minimumFreeSpaceBelow", self.minimum_free_space_below.to_string());
        field("windEnabled", self.wind_enabled.to_string());
        field("windlessDimensions", string_array(&self.windless_dimensions));
        let mut settings = String::new();
        for (id, s) in &self.leaf_settings {
            if *s == LeafSettings::defaults(id, class(id)) {
                continue;
            }
            let _ = write!(
                settings,
                "{}\n    \"{}\": {{\n      \"particleImplementation\": \"{}\",\n      \"spawnRateFactor\": {},\n      \"isConiferBlock\": {},\n      \"spawnBreakingLeaves\": {}\n    }}",
                if settings.is_empty() { "" } else { "," },
                json_escape(id),
                s.implementation.name(),
                java_float(s.spawn_rate_factor),
                s.is_conifer,
                s.spawn_breaking_leaves,
            );
        }
        field("leafSettings", if settings.is_empty() { "{}".into() } else { format!("{{{settings}\n  }}") });
        field("leafSpawners", string_array(&self.leaf_spawners));
        field("fallSpawnRateFactor", java_float(self.fall_spawn_rate_factor));
        field("winterSpawnRateFactor", java_float(self.winter_spawn_rate_factor));
        field("startingSpawnRadius", self.starting_spawn_radius.to_string());
        field("decaySpawnRateFactor", java_float(self.decay_spawn_rate_factor));
        field("maxDecayLeaves", self.max_decay_leaves.to_string());
        let _ = writeln!(o, "  \"registerParticles\": {}", self.register_particles);
        o.push('}');
        o
    }
}

fn json_escape(s: &str) -> String {
    serde_json::to_string(s).map_or_else(|_| s.to_owned(), |q| q[1..q.len() - 1].to_owned())
}

fn string_array(items: &[String]) -> String {
    if items.is_empty() {
        return "[]".into();
    }
    let inner: Vec<String> = items.iter().map(|s| format!("    \"{}\"", json_escape(s))).collect();
    format!("[\n{}\n  ]", inner.join(",\n"))
}

/// `Float.toString` for the values a person types: `1.8`, `1.0`.
pub fn java_float(f: f32) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1.0e7 { format!("{f:.1}") } else { format!("{f}") }
}

static CONFIG: RwLock<Option<Config>> = RwLock::new(None);
/// The mod is on in Options > Mods (the glue tells it each tick).
static ACTIVE: AtomicBool = AtomicBool::new(true);

pub fn set_active(on: bool) {
    ACTIVE.store(on, Ordering::Relaxed);
}

/// The window's particle engine: single player's server runs a game of its
/// own, whose ambient tick draws nothing, and the mod leaves that one alone.
static CLIENT_ENGINE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn set_client_engine(engine: &ParticleEngine) {
    CLIENT_ENGINE.store(engine as *const ParticleEngine as usize, Ordering::Relaxed);
}
fn is_client(engine: &ParticleEngine) -> bool {
    CLIENT_ENGINE.load(Ordering::Relaxed) == engine as *const ParticleEngine as usize
}

/// The settings now.
pub fn config() -> Config {
    CONFIG.read().ok().and_then(|c| c.clone()).unwrap_or_default()
}
pub fn set_config(c: Config) {
    if let Ok(mut slot) = CONFIG.write() {
        *slot = Some(c);
    }
}
fn with_config<R>(f: impl FnOnce(&Config) -> R) -> R {
    match CONFIG.read() {
        Ok(c) => match c.as_ref() {
            Some(c) => f(c),
            None => f(&Config::default()),
        },
        Err(_) => f(&Config::default()),
    }
}

// ---- the wind ---------------------------------------------------------------------------

/// `TriangularDistribution`: from `a` to `b`, most likely `c`.
#[derive(Clone, Copy, Debug)]
struct Triangular {
    a: f32,
    b: f32,
    c: f32,
}
impl Triangular {
    fn sample(self, rng: &mut LegacyRandom) -> f32 {
        let f = (self.c - self.a) / (self.b - self.a);
        let u = rng.next_float();
        if u < f {
            self.a + (u * (self.b - self.a) * (self.c - self.a)).sqrt()
        } else {
            self.b - ((1.0 - u) * (self.b - self.a) * (self.b - self.c)).sqrt()
        }
    }
}

/// `Wind.State`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Calm,
    Windy,
    Stormy,
}
impl State {
    fn speed(self) -> Triangular {
        match self {
            State::Calm => Triangular { a: 0.05, b: 0.2, c: 0.05 },
            State::Windy => Triangular { a: 0.05, b: 0.7, c: 0.3 },
            State::Stormy => Triangular { a: 0.05, b: 1.1, c: 0.6 },
        }
    }
}

/// `SmoothNoise`: values `interval` ticks apart, eased between.
#[derive(Clone, Copy, Debug)]
struct Noise {
    left: f32,
    right: f32,
    ticks: i32,
    t: f32,
    interval: i32,
}
impl Noise {
    fn new(interval: i32, initial: f32, next: f32) -> Noise {
        Noise { left: initial, right: next, ticks: 0, t: 0.0, interval }
    }
    /// One tick; `next` gives the next value when the current span ends.
    fn tick(&mut self, next: impl FnOnce(f32) -> f32) {
        self.ticks += 1;
        if self.ticks == self.interval {
            self.ticks = 0;
            self.left = self.right;
            self.right = next(self.left);
        }
        self.t = self.ticks as f32 / self.interval as f32;
    }
    fn lerp(&self) -> f32 {
        self.left + self.t * (self.right - self.left)
    }
    fn smooth(&self) -> f32 {
        let s = self.t * self.t * (3.0 - 2.0 * self.t);
        self.left + s * (self.right - self.left)
    }
}

/// `Wind`.
#[derive(Clone, Debug)]
pub struct Wind {
    rng: LegacyRandom,
    velocity: Noise,
    trend: Noise,
    direction: Noise,
    was_raining: bool,
    was_thundering: bool,
    state: State,
    original: Option<State>,
    duration: i32,
    pub x: f32,
    pub z: f32,
}

const TAU: f32 = std::f32::consts::TAU;

impl Wind {
    /// `Wind.init`.
    pub fn new(seed: i64) -> Wind {
        let mut rng = LegacyRandom::new(seed);
        let state = State::Calm;
        let velocity = Noise::new(2 * 20, 0.0, state.speed().sample(&mut rng));
        let trend_start = rng.next_float() * TAU;
        let trend = Noise::new(30 * 60 * 20, trend_start, rng.next_float() * TAU);
        let direction = Noise::new(10 * 20, 0.0, (2.0 * rng.next_float() - 1.0) * TAU / 8.0);
        Wind {
            rng,
            velocity,
            trend,
            direction,
            was_raining: false,
            was_thundering: false,
            state,
            original: None,
            duration: 0,
            x: 0.0,
            z: 0.0,
        }
    }

    /// `Wind.tickState`.
    fn tick_state(&mut self, windless: bool, raining: bool, thundering: bool) {
        self.duration -= 1;
        if windless {
            self.original = Some(self.state);
            self.state = State::Calm;
            return;
        }
        if let Some(s) = self.original.take() {
            self.state = s;
        }
        let changed = self.was_raining != raining || self.was_thundering != thundering;
        if changed || self.duration <= 0 {
            self.state = if thundering {
                State::Stormy
            } else {
                // Windy or stormy in rain, calm or windy otherwise.
                let i = self.rng.next_int_bound(2).unwrap_or(0);
                match (raining, i) {
                    (false, 0) => State::Calm,
                    (true, 1) => State::Stormy,
                    _ => State::Windy,
                }
            };
            self.duration = 6 * 60 * 20;
        }
        self.was_raining = raining;
        self.was_thundering = thundering;
    }

    /// `Wind.tick`: `windless` when wind is off or the dimension has none.
    pub fn tick(&mut self, windless: bool, raining: bool, thundering: bool) {
        self.tick_state(windless, raining, thundering);
        let (state, rng) = (self.state, &mut self.rng);
        self.velocity.tick(|_| state.speed().sample(rng));
        self.trend.tick(|_| rng.next_float() * TAU);
        self.direction.tick(|_| (2.0 * rng.next_float() - 1.0) * TAU / 8.0);
        let strength = self.velocity.smooth();
        let direction = self.trend.lerp() + self.direction.smooth();
        self.x = strength * horizon_core::geometry::trig::cos(f64::from(direction));
        self.z = strength * horizon_core::geometry::trig::sin(f64::from(direction));
    }
}

// ---- where leaves start -----------------------------------------------------------------

/// A particle to make, once the glue knows its colour.
#[derive(Clone, Copy, Debug)]
pub struct Request {
    pub kind: Kind,
    /// The block it comes from, for its colour.
    pub state: BlockStateId,
    pub at: [f64; 3],
}

static QUEUE: Mutex<Vec<Request>> = Mutex::new(Vec::new());

pub fn take_requests() -> Vec<Request> {
    QUEUE.lock().map(|mut q| std::mem::take(&mut *q)).unwrap_or_default()
}
fn request(r: Request) {
    if let Ok(mut q) = QUEUE.lock() {
        q.push(r);
    }
}

/// The player's block (`player.blockPosition()`), for the minimal spawn radius.
static PLAYER: [AtomicI32; 3] = [AtomicI32::new(0), AtomicI32::new(i32::MIN), AtomicI32::new(0)];

pub fn set_player(p: Option<[i32; 3]>) {
    let p = p.unwrap_or([0, i32::MIN, 0]);
    for (slot, v) in PLAYER.iter().zip(p) {
        slot.store(v, Ordering::Relaxed);
    }
}
fn player() -> Option<[i32; 3]> {
    let p = [0, 1, 2].map(|i| PLAYER[i].load(Ordering::Relaxed));
    (p[1] != i32::MIN).then_some(p)
}

/// The level's blocks, as the ambient tick and the game both reach them.
pub struct Area<'a> {
    pub world: &'a World,
    pub facts: &'a BlockFacts,
    pub collision: &'a BlockCollisionRegistry<'a>,
    pub registry: &'a BlockRegistry,
    pub biomes: &'a Biomes,
}

impl Area<'_> {
    fn state(&self, p: [i32; 3]) -> BlockStateId {
        self.world.block(p[0], p[1], p[2])
    }
    fn id(&self, state: BlockStateId) -> String {
        full_id(&self.facts.get(state).name)
    }
    /// `isLeafBlock(block, true)`: a `LeavesBlock`, or in `#minecraft:leaves`.
    fn is_leaves(&self, state: BlockStateId) -> bool {
        let facts = self.facts.get(state);
        facts.has_class("LeavesBlock") || facts.name.ends_with("_leaves")
    }
    fn property(&self, state: BlockStateId, key: &str) -> Option<String> {
        self.registry.state(state)?.properties.get(key).cloned()
    }
    /// `getBlockCollisions(null, box).hasNext()`: a block's collision shape
    /// overlaps the box.
    fn collides(&self, min: [f64; 3], max: [f64; 3]) -> bool {
        let lo = min.map(|v| v.floor() as i32);
        let hi = max.map(|v| v.floor() as i32);
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let state = self.state([x, y, z]);
                    let Ok(Some(shape)) = self.collision.cached_shapes().cached_shape(state) else {
                        continue;
                    };
                    let at = [f64::from(x), f64::from(y), f64::from(z)];
                    let hit = shape.boxes().iter().any(|b| {
                        let (bmin, bmax) = (b.min_position(), b.max_position());
                        bmin.x + at[0] < max[0]
                            && bmax.x + at[0] > min[0]
                            && bmin.y + at[1] < max[1]
                            && bmax.y + at[1] > min[1]
                            && bmin.z + at[2] < max[2]
                            && bmax.z + at[2] > min[2]
                    });
                    if hit {
                        return true;
                    }
                }
            }
        }
        false
    }
    /// The biome's name at a block.
    fn biome(&self, p: [i32; 3]) -> &str {
        let biome = self.world.chunks.get(&(p[0] >> 4, p[2] >> 4)).map_or(0, |c| c.biome(p[0], p[1], p[2]));
        &self.biomes.get(biome).name
    }
}

/// What a block is to Falling Leaves, from the game's own tables.
pub fn classify(tables: &AnimateTables, facts: &BlockFacts, state: BlockStateId) -> LeafClass {
    match tables.kind(state) {
        Animate::Leaves(Some((T::TintedLeaves, chance)), _, _) => LeafClass::Tinted(chance),
        Animate::Leaves(Some((_, chance)), _, _) => LeafClass::Untinted(chance),
        // Spruce: a tinted leaves block with no falling leaves of its own.
        Animate::Leaves(None, _, _) => LeafClass::Tinted(0.0),
        _ if facts.get(state).has_class("LeavesBlock") => LeafClass::OtherLeaves,
        _ => LeafClass::NotLeaves,
    }
}

/// Which of the engine's generators a spawn draws from: the ambient tick's
/// (`animateTick`'s random) or the level's (`level.getRandom()`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rng {
    Animate,
    Level,
}
fn rng(engine: &mut ParticleEngine, which: Rng) -> &mut LegacyRandom {
    match which {
        Rng::Animate => engine.random(),
        Rng::Level => engine.level_random(),
    }
}

/// Chebyshev distance (`getMaximumDistance`).
fn max_distance(a: [i32; 3], b: [i32; 3]) -> i32 {
    (0..3).map(|i| (a[i] - b[i]).abs()).max().unwrap_or(0)
}
fn inside_minimal_radius(cfg: &Config, pos: [i32; 3]) -> bool {
    cfg.starting_spawn_radius != 0 && player().is_some_and(|p| max_distance(p, pos) < cfg.starting_spawn_radius)
}

/// `getModifiedSpawnChance`.
fn modified_spawn_chance(cfg: &Config, area: &Area<'_>, pos: [i32; 3], state: BlockStateId, s: Option<LeafSettings>) -> f32 {
    if inside_minimal_radius(cfg, pos) {
        return 0.0;
    }
    if !cfg.drop_from_player_placed_blocks && area.property(state, "persistent").as_deref() == Some("true") {
        return 0.0;
    }
    let Some(s) = s else {
        return 0.0;
    };
    let mut chance = s.spawn_chance(cfg);
    // No seasons in Horizon: neither the fall nor the winter factor applies.
    if cfg.decay_spawn_rate_factor != 1.0 && area.is_leaves(state) && area.facts.get(state).random_ticks {
        chance *= cfg.decay_spawn_rate_factor;
    }
    chance
}

/// The block's vanilla falling leaf (`spawnFallingLeavesParticle`), as the
/// game's ambient tick makes it.
fn vanilla_leaf(engine: &mut ParticleEngine, area: &Area<'_>, tables: &AnimateTables, state: BlockStateId, pos: [i32; 3], which: Rng) {
    let (ty, fixed_tint) = match tables.kind(state) {
        Animate::Leaves(Some((ty, _)), tint, _) => (ty, tint),
        Animate::Leaves(None, tint, _) => (T::TintedLeaves, tint),
        _ => return,
    };
    let r = rng(engine, which);
    let p = [f64::from(pos[0]) + r.next_double(), f64::from(pos[1]) - 0.05, f64::from(pos[2]) + r.next_double()];
    let options = if ty == T::TintedLeaves {
        let c = fixed_tint.unwrap_or_else(|| {
            let biome = area.world.chunks.get(&(pos[0] >> 4, pos[2] >> 4)).map_or(0, |c| c.biome(pos[0], pos[1], pos[2]));
            area.biomes.get(biome).foliage
        });
        let f = |s: u32| ((c >> s) & 255) as f32 / 255.0;
        ParticleOptions::colored(ty, [f(16), f(8), f(0), 1.0])
    } else {
        ParticleOptions::simple(ty)
    };
    engine.add(&options, p, [0.0; 3]);
}

/// `spawnLeafParticles`.
#[allow(clippy::too_many_arguments)]
pub fn spawn_leaf_particles(
    cfg: &Config,
    engine: &mut ParticleEngine,
    area: &Area<'_>,
    tables: &AnimateTables,
    count: i32,
    inside: bool,
    state: BlockStateId,
    pos: [i32; 3],
    which: Rng,
    s: LeafSettings,
) {
    if count == 0 {
        return;
    }
    let implementation = s.implementation(cfg);
    let falling = matches!(tables.kind(state), Animate::Leaves(..));
    if implementation == Implementation::Vanilla && falling {
        for _ in 0..count {
            vanilla_leaf(engine, area, tables, state, pos, which);
        }
        return;
    }
    let kind = match implementation {
        Implementation::Cherry => Kind::Cherry,
        Implementation::Conifer => Kind::Conifer,
        Implementation::Regular | Implementation::Vanilla => Kind::Leaf,
    };
    spawn_particles(cfg, engine, area, count, inside, kind, state, pos, which);
}

/// `spawnSnowParticles`: only from trees in snowy biomes or under snow.
#[allow(clippy::too_many_arguments)]
pub fn spawn_snow_particles(
    cfg: &Config,
    engine: &mut ParticleEngine,
    area: &Area<'_>,
    count: i32,
    inside: bool,
    state: BlockStateId,
    pos: [i32; 3],
    which: Rng,
) {
    if count == 0 {
        return;
    }
    // Every snowy vanilla biome (no seasons, so never summer).
    let mut snowy = crate::mobs::finalize::villager_type(area.biome(pos)) == 4;
    if !snowy {
        // The top of the column: a snow layer, snow or powder snow.
        let top = area.world.world_surface(pos[0], pos[2], |s| area.facts.is_air(s)) - 1;
        let facts = area.facts.get(area.state([pos[0], top, pos[2]]));
        snowy = facts.has_class("SnowLayerBlock")
            || facts.has_class("PowderSnowBlock")
            || matches!(&*full_id(&facts.name), "minecraft:snow" | "minecraft:snow_block" | "minecraft:powder_snow");
    }
    if snowy {
        spawn_particles(cfg, engine, area, count, inside, Kind::Snow, state, pos, which);
    }
}

/// `spawnParticles`: where each starts, below the block (with room to fall)
/// or inside it.
#[allow(clippy::too_many_arguments)]
fn spawn_particles(
    cfg: &Config,
    engine: &mut ParticleEngine,
    area: &Area<'_>,
    count: i32,
    inside: bool,
    kind: Kind,
    state: BlockStateId,
    pos: [i32; 3],
    which: Rng,
) {
    for _ in 0..count {
        let r = rng(engine, which);
        let x = f64::from(pos[0]) + r.next_double();
        let z = f64::from(pos[2]) + r.next_double();
        let y = if inside {
            f64::from(pos[1]) + r.next_double()
        } else {
            // Below an opaque block, so the leaf isn't drawn black inside it.
            let y = f64::from(pos[1]) - if area.facts.get(state).can_occlude { 0.1 } else { 0.0 };
            if !has_room(cfg, area, pos, x, y, z) {
                continue;
            }
            y
        };
        request(Request { kind, state, at: [x, y, z] });
    }
}

/// `hasRoomForLeafParticle`: no leaves right below, and nothing solid in the
/// free space it needs.
fn has_room(cfg: &Config, area: &Area<'_>, pos: [i32; 3], x: f64, y: f64, z: f64) -> bool {
    if area.is_leaves(area.state([pos[0], pos[1] - 1, pos[2]])) {
        return false;
    }
    let y2 = y - f64::from(cfg.minimum_free_space_below) * 0.5;
    !area.collides([x - 0.1, y2, z - 0.1], [x + 0.1, y, z + 0.1])
}

/// `trySpawnSnowParticle`.
fn try_snow(cfg: &Config, engine: &mut ParticleEngine, area: &Area<'_>, state: BlockStateId, pos: [i32; 3]) {
    if inside_minimal_radius(cfg, pos) {
        return;
    }
    let chance = f64::from(cfg.snowflake_spawn_chance());
    if chance != 0.0 && engine.random().next_double() < chance {
        spawn_snow_particles(cfg, engine, area, 1, false, state, pos, Rng::Animate);
    }
}

/// `animateTick` of a block, before the game's own: true when Falling Leaves
/// did the leaves' whole tick (`LeafTickMixin`, `LeafTickMixin2`), false to let
/// the game go on (after the extra leaves of `LeafTickMixin` for other leaves
/// and `BlockMixin` for the spawner blocks).
pub fn block_tick(tables: &AnimateTables, engine: &mut ParticleEngine, ctx: &AnimateContext, state: BlockStateId, pos: [i32; 3]) -> bool {
    with_config(|cfg| {
        if !cfg.enabled || !ACTIVE.load(Ordering::Relaxed) || !is_client(engine) {
            return false;
        }
        let area = Area {
            world: ctx.world,
            facts: ctx.facts,
            collision: ctx.collision,
            registry: ctx.registry,
            biomes: ctx.biomes,
        };
        let class = classify(tables, ctx.facts, state);
        let id = area.id(state);
        match tables.kind(state) {
            Animate::Leaves(falling, fixed_tint, poplar) => {
                leaves_tick(cfg, engine, ctx, &area, tables, state, pos, &id, class, falling, fixed_tint, poplar);
                true
            }
            _ if class == LeafClass::OtherLeaves => {
                // LeafTickMixin: a leaves block without falling leaves of its own.
                if let Some(s) = cfg.leaf_settings(&id, class) {
                    let chance = modified_spawn_chance(cfg, &area, pos, state, Some(s));
                    if chance != 0.0 && engine.random().next_float() < chance {
                        spawn_leaf_particles(cfg, engine, &area, tables, 1, false, state, pos, Rng::Animate, s);
                    }
                }
                try_snow(cfg, engine, &area, state, pos);
                false
            }
            _ if cfg.is_leaf_spawner(&id) => {
                // BlockMixin: a block from "Additional block states that drop leaves".
                let matches = cfg
                    .spawner_properties(&id)
                    .iter()
                    .all(|(k, v)| area.property(state, k).is_none_or(|have| &have == v));
                if matches {
                    try_snow(cfg, engine, &area, state, pos);
                    if let Some(s) = cfg.leaf_settings(&id, class) {
                        let chance = modified_spawn_chance(cfg, &area, pos, state, Some(s));
                        if chance != 0.0 && engine.random().next_float() < chance {
                            spawn_leaf_particles(cfg, engine, &area, tables, 1, false, state, pos, Rng::Animate, s);
                        }
                    }
                }
                false
            }
            _ => false,
        }
    })
}

/// `FallingParticlesLeavesBlock.animateTick` with Falling Leaves: the snow,
/// the game's dripping water and poplar rustle, then the falling leaf at the
/// mod's chance, the mod's own unless the block's implementation is vanilla's.
#[allow(clippy::too_many_arguments)]
fn leaves_tick(
    cfg: &Config,
    engine: &mut ParticleEngine,
    ctx: &AnimateContext,
    area: &Area<'_>,
    tables: &AnimateTables,
    state: BlockStateId,
    pos: [i32; 3],
    id: &str,
    class: LeafClass,
    _falling: Option<(T, f32)>,
    _fixed_tint: Option<u32>,
    poplar: bool,
) {
    let (fx, fy, fz) = (f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]));
    let below = [pos[0], pos[1] - 1, pos[2]];
    // LeafTickMixin at the head of LeavesBlock.animateTick.
    try_snow(cfg, engine, area, state, pos);
    // LeavesBlock.makeDrippingWaterParticles.
    if (ctx.raining_at)([pos[0], pos[1] + 1, pos[2]]) && next_int(engine.random(), 15) == 1 {
        let below_facts = ctx.facts.get(ctx.state(below));
        if !below_facts.can_occlude || !ctx.sturdy(below) {
            let r = engine.random();
            let p = [fx + r.next_double(), fy - 0.05, fz + r.next_double()];
            add(engine, T::DrippingWater, p, [0.0; 3]);
        }
    }
    if poplar {
        leaves_ambient_sound(tables, engine, ctx, state, pos);
    }
    // FallingParticlesLeavesBlock.makeFallingLeavesParticles, its chance the mod's.
    let s = cfg.leaf_settings(id, class);
    let chance = modified_spawn_chance(cfg, area, pos, state, s);
    if engine.random().next_float() < chance {
        let Some(s) = s else {
            return;
        };
        if s.implementation(cfg) == Implementation::Vanilla {
            // The game's own leaf, under its own check for a full face below.
            if !ctx.sturdy(below) {
                vanilla_leaf(engine, area, tables, state, pos, Rng::Animate);
            }
        } else {
            // The mod checks the room itself (hasRoomForLeafParticle).
            spawn_leaf_particles(cfg, engine, area, tables, 1, false, state, pos, Rng::Animate, s);
        }
    }
}

/// `registerAttackBlockLeaves`: 0 to 3 leaves and a little snow from a leaf
/// block that is hit.
pub fn hit(engine: &mut ParticleEngine, area: &Area<'_>, tables: &AnimateTables, state: BlockStateId, pos: [i32; 3]) {
    with_config(|cfg| {
        if !cfg.enabled || !ACTIVE.load(Ordering::Relaxed) || !cfg.leaves_on_block_hit {
            return;
        }
        let class = classify(tables, area.facts, state);
        let Some(s) = cfg.leaf_settings(&area.id(state), class) else {
            return;
        };
        if s.spawn_breaking_leaves {
            let count = (0..3).filter(|_| engine.level_random().next_boolean()).count() as i32;
            spawn_leaf_particles(cfg, engine, area, tables, count, false, state, pos, Rng::Level, s);
        }
        if cfg.snowflake_spawn_chance() != 0.0 {
            let count = (0..6).filter(|_| engine.level_random().next_boolean()).count() as i32;
            spawn_snow_particles(cfg, engine, area, count, false, state, pos, Rng::Level);
        }
    });
}

/// `addDecayingLeafParticles`: a leaf block that decayed (single player).
pub fn decayed(engine: &mut ParticleEngine, area: &Area<'_>, tables: &AnimateTables, state: BlockStateId, pos: [i32; 3]) {
    with_config(|cfg| {
        if !cfg.enabled || !ACTIVE.load(Ordering::Relaxed) || cfg.max_decay_leaves == 0 {
            return;
        }
        let class = classify(tables, area.facts, state);
        if let Some(s) = cfg.leaf_settings(&area.id(state), class)
            && s.spawn_breaking_leaves
        {
            let count = (0..cfg.max_decay_leaves).filter(|_| engine.level_random().next_boolean()).count() as i32;
            spawn_leaf_particles(cfg, engine, area, tables, count, true, state, pos, Rng::Level, s);
        }
        let snow = (0..2 * cfg.max_decay_leaves).filter(|_| engine.level_random().next_boolean()).count() as i32;
        spawn_snow_particles(cfg, engine, area, snow, true, state, pos, Rng::Level);
    });
}

/// Leaf blocks that decayed on the server since the last tick, with their state.
static DECAYED: Mutex<Vec<(BlockStateId, [i32; 3])>> = Mutex::new(Vec::new());

pub fn push_decayed(state: BlockStateId, pos: [i32; 3]) {
    if let Ok(mut d) = DECAYED.lock() {
        d.push((state, pos));
    }
}
pub fn take_decayed() -> Vec<(BlockStateId, [i32; 3])> {
    DECAYED.lock().map(|mut d| std::mem::take(&mut *d)).unwrap_or_default()
}
