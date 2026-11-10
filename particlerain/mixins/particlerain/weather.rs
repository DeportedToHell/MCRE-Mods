// Particle Rain's weather in the game: where its particles start (`ParticleSpawner`: in the sky
// round the camera, on the world's surface, and on the faces of the blocks the ambient tick
// samples), its particles' tick (`CustomParticle`, `BlockDisplayParticle`, `StreakParticle`), the
// wind (`ParticleRain.getWind`, `WindManager`), the tints (`TintType`), the game's weather
// splashes and rain sound as the original changes them (`ClientLevelMixin`: the snow and
// sandstorm sounds, the mod's rain volume, no splashes unless asked for), the "after weather"
// time (`WeatherDataMixin`), and the frame's quads (each `RotationType`, block models for
// `BLOCK_MODEL`). Spliced into game.rs as `particlerain_weather`.
//
// Particle Rain by PigCart (https://github.com/PigCart/particle-rain), MIT.

use super::Game;
use super::particlerain_core::{self as core, Config, FadeType, ParticleData, ParticleStyle, Precipitation, RenderType, RotationType, SpawnPos, TintType};
use super::particlerain_glue as glue;
use horizon_client::particles::particlerain_particle as pp;
use horizon_client::particles::{ParticleOptions, ParticleType};
use horizon_core::geometry::{Direction, Vec3, traverse_blocks};
use horizon_core::position::BlockPos;
use horizon_core::random::{LegacyRandom, RandomSource};
use horizon_world::registry::BlockStateId;
use std::collections::HashMap;
use std::sync::Mutex;

const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;
/// `VersionUtil.getBiomeFogColor` from 1.21.11 on: a fixed sky blue.
const FOG_COLOR: [f32; 3] = [185.0, 211.0, 251.0];
/// The bundle's sounds, for `particlerain_sound`.
const SOUNDS: &[(&str, &[u8])] = &[
    ("snow-01", horizon_client::native_data!("particlerain", "snow-01.ogg")),
    ("snow-02", horizon_client::native_data!("particlerain", "snow-02.ogg")),
    ("snow-03", horizon_client::native_data!("particlerain", "snow-03.ogg")),
    ("snow-04", horizon_client::native_data!("particlerain", "snow-04.ogg")),
    ("snow-05", horizon_client::native_data!("particlerain", "snow-05.ogg")),
    ("snow-06", horizon_client::native_data!("particlerain", "snow-06.ogg")),
    ("sandstorm-01", horizon_client::native_data!("particlerain", "sandstorm-01.ogg")),
    ("sandstorm-02", horizon_client::native_data!("particlerain", "sandstorm-02.ogg")),
    ("sandstorm-03", horizon_client::native_data!("particlerain", "sandstorm-03.ogg")),
    ("sandstorm-04", horizon_client::native_data!("particlerain", "sandstorm-04.ogg")),
    ("sandstorm-05", horizon_client::native_data!("particlerain", "sandstorm-05.ogg")),
    ("sandstorm-06", horizon_client::native_data!("particlerain", "sandstorm-06.ogg")),
    ("sandstormabove-01", horizon_client::native_data!("particlerain", "sandstormabove-01.ogg")),
    ("sandstormabove-02", horizon_client::native_data!("particlerain", "sandstormabove-02.ogg")),
    ("sandstormabove-03", horizon_client::native_data!("particlerain", "sandstormabove-03.ogg")),
    ("sandstormabove-04", horizon_client::native_data!("particlerain", "sandstormabove-04.ogg")),
    ("sandstormabove-05", horizon_client::native_data!("particlerain", "sandstormabove-05.ogg")),
    ("sandstormabove-06", horizon_client::native_data!("particlerain", "sandstormabove-06.ogg")),
];

// ---- the particles ---------------------------------------------------------------------------

/// What a particle is drawn as.
#[derive(Clone, Debug)]
enum Look {
    /// `CustomParticle`: a quad of its sprite.
    Quad,
    /// `BlockDisplayParticle`: a block's model.
    Block(BlockStateId),
    /// `StreakParticle`: a streak running down a wall facing `face`, while the wall is in `list`.
    Streak { face: Direction, list: core::Whitelist },
}

/// One of the mod's particles (`WeatherParticle` and its kinds).
#[derive(Clone, Debug)]
struct Drop {
    /// Its settings (`ParticleData`), by id; `at` is where they were last found.
    id: String,
    at: usize,
    look: Look,
    pos: [f64; 3],
    prev: [f64; 3],
    vel: [f64; 3],
    age: i32,
    lifetime: i32,
    gravity: f32,
    removed: bool,
    quad_size: f32,
    o_quad_size: f32,
    base_size: f32,
    roll: f32,
    o_roll: f32,
    rotation_variation: f32,
    alpha: f32,
    rgb: [f32; 3],
    /// The atlas name of its sprite.
    sprite: String,
    block: [i32; 3],
    o_block: [i32; 3],
    base_temp: f32,
    collision_anim: bool,
    progress: f32,
    o_progress: f32,
    speed: f32,
    distance: f32,
    edge_bounces: i32,
    /// `BlockDisplayParticle`'s entity yaw (degrees), now and a tick ago.
    yaw: f32,
    o_yaw: f32,
    /// Its own random (`Particle.random`).
    random: LegacyRandom,
}

/// `ParticleSpawner`'s and `WindManager`'s state, and the live particles.
struct Runtime {
    drops: Vec<Drop>,
    /// `ParticleSpawner.RANDOM`.
    random: LegacyRandom,
    /// `level.getRandom()` as the particles draw from it.
    level_random: LegacyRandom,
    seeds: LegacyRandom,
    after_weather_ticks_left: i32,
    spawn_attempts_until_block_fx_idle: i32,
    ticks_until_surface_fx_idle: i32,
    ticks_until_sky_fx_idle: i32,
    heights: HashMap<(i32, i32), i32>,
    last_tick: i64,
    /// `isRaining` last tick (`WeatherData.setRaining`'s change).
    raining: Option<bool>,
    /// The field of view the frame drew with, for `distanceScaling` 1.
    fov: f32,
}

static RUNTIME: Mutex<Option<Runtime>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Runtime) -> R) -> R {
    let mut guard = RUNTIME.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let rt = guard.get_or_insert_with(|| {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as i64);
        Runtime {
            drops: Vec::new(),
            random: LegacyRandom::new(seed),
            level_random: LegacyRandom::new(seed ^ 0x5DEE_CE66),
            seeds: LegacyRandom::new(seed.rotate_left(17)),
            after_weather_ticks_left: 0,
            spawn_attempts_until_block_fx_idle: 0,
            ticks_until_surface_fx_idle: 0,
            ticks_until_sky_fx_idle: 0,
            heights: HashMap::new(),
            last_tick: i64::MIN,
            raining: None,
            fov: 70.0,
        }
    });
    f(rt)
}

/// `ParticleEngine.clearParticles` (a new level, or `ClearParticles`): the mod's particles go.
pub(super) fn clear() {
    with(|rt| {
        rt.drops.clear();
        rt.heights.clear();
        rt.raining = None;
    });
}

/// `ParticleSpawner.particleCount`.
pub(super) fn count() -> usize {
    with(|rt| rt.drops.len())
}

/// `ParticleSpawner.getHeight` at a column, for `/particlerain debug`.
pub(super) fn height_at(game: &Game<'_>, x: i32, z: i32) -> i32 {
    let eye = game.eye_position().unwrap_or([0.0; 3]);
    glue::with(|st| {
        let tags = st.tags.get_or_insert_with(|| core::Tags::load(&game.vanilla_root()));
        let lv = Level {
            game,
            cfg: &st.config,
            particles: &st.particles,
            tags: &*tags,
            rain: 0.0,
            thunder: 0.0,
            thundering: false,
            camera: eye,
            camera_velocity: [0.0; 3],
            game_time: game.world.game_time,
        };
        with(|rt| rt.height(&lv, x, z))
    })
}

/// `ParticleSpawner.getSkyParticleDensity`, for `/particlerain debug` (it counts the idle time
/// down, as the original's does).
pub(super) fn sky_density(thunder: f32, speed: f32) -> i32 {
    let cfg = glue::with(|st| st.config.clone());
    with(|rt| rt.sky_density(&cfg, thunder, speed))
}

/// The spawner's counters, for `/particlerain debug`.
pub(super) fn counters() -> (i32, i32, i32, i32) {
    with(|rt| (rt.after_weather_ticks_left, rt.spawn_attempts_until_block_fx_idle, rt.ticks_until_sky_fx_idle, rt.ticks_until_surface_fx_idle))
}

// ---- the world as the mod reads it -----------------------------------------------------------

/// The level the tick works on: the game's world and the settings, read once.
struct Level<'g, 'a> {
    game: &'g Game<'a>,
    cfg: &'g Config,
    particles: &'g [ParticleData],
    tags: &'g core::Tags,
    rain: f32,
    thunder: f32,
    thundering: bool,
    camera: [f64; 3],
    camera_velocity: [f64; 3],
    game_time: i64,
}

fn floor3(p: [f64; 3]) -> [i32; 3] {
    p.map(|v| v.floor() as i32)
}

impl Level<'_, '_> {
    fn world(&self) -> &horizon_client::world::World {
        &self.game.world
    }
    fn biomes(&self) -> &horizon_client::biome::Biomes {
        match &self.game.world_biomes {
            Some(b) => b,
            None => self.game.res.biomes(),
        }
    }
    fn state(&self, b: [i32; 3]) -> BlockStateId {
        self.world().block(b[0], b[1], b[2])
    }
    fn facts(&self, s: BlockStateId) -> &horizon_client::blocks::StateFacts {
        self.game.res.facts().get(s)
    }
    fn block_name(&self, s: BlockStateId) -> String {
        core::full_id(&self.facts(s).name)
    }
    fn loaded(&self, b: [i32; 3]) -> bool {
        let w = self.world();
        b[1] >= w.bottom_y() && b[1] < w.top_y() && w.loaded(b[0], b[2])
    }
    /// The biome at a block (`level.getBiome`), as an index of `biomes()`.
    fn biome_id(&self, b: [i32; 3]) -> Option<u16> {
        self.world().chunks.get(&(b[0] >> 4, b[2] >> 4)).map(|c| c.biome(b[0], b[1], b[2]))
    }
    fn biome_name(&self, b: [i32; 3]) -> String {
        self.biome_id(b).map(|i| self.biomes().get(i).name.clone()).unwrap_or_default()
    }
    fn base_temperature(&self, b: [i32; 3]) -> f32 {
        self.biome_id(b).map_or(0.5, |i| self.biomes().get(i).temperature)
    }
    /// `Biome.getPrecipitationAt`.
    fn precipitation(&self, b: [i32; 3]) -> Precipitation {
        match self.biome_id(b).and_then(|i| self.biomes().get(i).snows_at(b[0], b[1], b[2])) {
            None => Precipitation::None,
            Some(true) => Precipitation::Snow,
            Some(false) => Precipitation::Rain,
        }
    }
    fn biome_ok(&self, list: &core::Whitelist, b: [i32; 3]) -> bool {
        let name = self.biome_name(b);
        list.contains(&name, &|tag| self.tags.biome_in(&name, tag))
    }
    fn block_ok(&self, list: &core::Whitelist, s: BlockStateId) -> bool {
        let name = self.block_name(s);
        list.contains(&name, &|tag| self.tags.block_in(&name, tag))
    }
    /// `ParticleSpawner.isIgnored`.
    fn ignored(&self, s: BlockStateId) -> bool {
        self.block_ok(&self.cfg.compat.weather_ignore_blocks, s)
    }
    /// `getHeight(Heightmap.Types.MOTION_BLOCKING, x, z)`.
    fn motion_blocking(&self, x: i32, z: i32) -> i32 {
        super::render_tick::motion_blocking_height(&self.game.world, &self.game.pack.assets.properties, x, z)
    }
    fn shape(&self, s: BlockStateId) -> Option<&horizon_core::geometry::VoxelShape> {
        self.game.collision.cached_shapes().cached_shape(s).ok().flatten().filter(|shape| !shape.is_empty())
    }
    fn no_collision(&self, s: BlockStateId) -> bool {
        self.shape(s).is_none() && !self.facts(s).full_collision
    }
    fn fluid(&self, s: BlockStateId) -> bool {
        self.facts(s).fluid != horizon_client::blocks::Fluid::None
    }
    /// `FluidState.getHeight`: a whole block under the same fluid, else its amount.
    fn fluid_height(&self, b: [i32; 3]) -> f64 {
        let f = self.facts(self.state(b));
        if f.fluid == horizon_client::blocks::Fluid::None {
            return 0.0;
        }
        if self.facts(self.state([b[0], b[1] + 1, b[2]])).fluid == f.fluid {
            1.0
        } else {
            f64::from(f.fluid_level) / 9.0
        }
    }
    /// `ParticleSpawner.calculateHeight`: down from the MOTION_BLOCKING height through the blocks
    /// weather ignores and those without collision, to the first other block or fluid.
    fn calculate_height(&self, x: i32, z: i32) -> i32 {
        let w = self.world();
        let mut y = self.motion_blocking(x, z);
        if y == 0 || y == -1 {
            y = w.top_y() - 1;
        }
        while y > w.bottom_y() {
            let s = self.state([x, y, z]);
            if !self.fluid(s) && (self.no_collision(s) || self.ignored(s)) {
                y -= 1;
            } else {
                break;
            }
        }
        y + 1
    }
    /// The mod's own `level.clip` (`ClipContext.Block.COLLIDER`, `Fluid.ANY`): the first block or
    /// fluid surface on the way, its block and the face hit.
    fn clip(&self, from: [f64; 3], to: [f64; 3]) -> Option<([i32; 3], Direction)> {
        let (from, to) = (Vec3::new(from[0], from[1], from[2]), Vec3::new(to[0], to[1], to[2]));
        if from == to {
            return None;
        }
        let hit = traverse_blocks(from, to, 64, |p: BlockPos| {
            let b = [p.x, p.y, p.z];
            let s = self.state(b);
            let block = match self.shape(s) {
                Some(shape) => shape.clip(from, to, p).map(|h| (h.location, h.direction)),
                None if self.facts(s).full_collision => {
                    let o = Vec3::new(f64::from(p.x), f64::from(p.y), f64::from(p.z));
                    face_hit(horizon_core::geometry::Aabb::new(o, Vec3::new(o.x + 1.0, o.y + 1.0, o.z + 1.0)), from, to)
                }
                None => None,
            };
            let fluid = if self.fluid(s) {
                let o = Vec3::new(f64::from(p.x), f64::from(p.y), f64::from(p.z));
                let h = self.fluid_height(b);
                face_hit(horizon_core::geometry::Aabb::new(o, Vec3::new(o.x + 1.0, o.y + h, o.z + 1.0)), from, to)
            } else {
                None
            };
            let best = match (block, fluid) {
                (Some(a), Some(c)) => Some(if from.distance_squared(a.0) <= from.distance_squared(c.0) { a } else { c }),
                (a, c) => a.or(c),
            };
            best.map(|(_, d)| (b, d))
        });
        hit.ok().flatten()
    }
}

/// Where a segment enters a box, and the face it crosses there.
fn face_hit(bounds: horizon_core::geometry::Aabb, from: Vec3, to: Vec3) -> Option<(Vec3, Direction)> {
    let at = bounds.clip(from, to)?;
    let (lo, hi) = (bounds.min_position(), bounds.max_position());
    let faces = [
        ((at.x - lo.x).abs(), Direction::West),
        ((at.x - hi.x).abs(), Direction::East),
        ((at.y - lo.y).abs(), Direction::Down),
        ((at.y - hi.y).abs(), Direction::Up),
        ((at.z - lo.z).abs(), Direction::North),
        ((at.z - hi.z).abs(), Direction::South),
    ];
    let face = faces.iter().min_by(|a, b| a.0.total_cmp(&b.0)).map_or(Direction::Up, |f| f.1);
    Some((at, face))
}

/// `Mth.sin`.
fn msin(v: f32) -> f32 {
    horizon_core::geometry::trig::sin(f64::from(v))
}
fn mcos(v: f32) -> f32 {
    horizon_core::geometry::trig::cos(f64::from(v))
}

/// `ParticleRain.yLevelWindMultiplier`.
pub(super) fn y_level_wind_multiplier(y: f64) -> f32 {
    ((y - 50.0) / 40.0).clamp(0.0, 1.0) as f32
}

/// `ParticleRain.oneWindPlease`.
fn one_wind(pos: f32, frequency: f32, time: f32, max: f32) -> f32 {
    -(msin(pos * frequency + time) * msin(pos * frequency * 0.5 + time * 0.5) * max + max)
}

/// `ParticleRain.getWind`: the wind's x and z at a place (none without the y-level adjustment,
/// as the original's multiplier is 0 then).
fn wind_at(cfg: &Config, game_time: i64, p: [f64; 3]) -> [f32; 2] {
    let w = &cfg.wind;
    let shift = game_time as f32 * w.modulation_speed;
    let multiplier = if w.y_level_adjustment { y_level_wind_multiplier(p[1]) } else { 0.0 };
    let wind = [
        one_wind(p[0] as f32, w.gust_frequency, shift, w.strength) * multiplier,
        one_wind(p[2] as f32, w.gust_frequency, shift, w.strength) * multiplier,
    ];
    if !w.changing_direction {
        return wind;
    }
    // The port's changing wind direction: the original's wind turned round by the angle the game
    // time gives, the same for every player of a world.
    let (s, c) = wind_angle(game_time).sin_cos();
    [wind[0] * c - wind[1] * s, wind[0] * s + wind[1] * c]
}

/// The port's addition: how far the wind has turned from the original's north-west, from the game
/// time alone: three slow sines with unrelated periods (about 7, 13 and 19 minutes), together
/// wandering through every direction and back.
fn wind_angle(game_time: i64) -> f32 {
    use std::f64::consts::{PI, TAU};
    let t = game_time as f64;
    let a = PI * (TAU * t / 8_440.0).sin() + 0.9 * PI * (TAU * t / 15_730.0 + 1.3).sin() + 0.6 * PI * (TAU * t / 22_910.0 + 2.1).sin();
    a as f32
}

/// `WindManager.windMultiplier`.
fn wind_multiplier(thundering: bool, data: &ParticleData) -> f32 {
    if thundering { data.storm_wind_strength } else { data.wind_strength }
}

/// `ParticleData.TintType.applyTint`: the colour a particle takes at `b`, if any.
fn tint(game: &Game<'_>, lv_rain: f32, data: &ParticleData, cfg: &Config, b: [i32; 3], random: &mut LegacyRandom) -> Option<[f32; 3]> {
    match data.tint_type {
        TintType::Water => water_tint(game, cfg, b),
        TintType::Fog => {
            let mul = 0.7 + ((-lv_rain + 1.0) * 0.2);
            Some(FOG_COLOR.map(|c| c * mul / 255.0))
        }
        TintType::Map => {
            // The block under the MOTION_BLOCKING height there, its map colour.
            let top = super::render_tick::motion_blocking_height(&game.world, &game.pack.assets.properties, b[0], b[2]);
            let s = game.world.block(b[0], top - 1, b[2]);
            let id = usize::from(game.res.facts().get(s).map_color);
            let col = horizon_client::blocks::MAP_COLORS.get(id).copied().unwrap_or(0);
            let variance = random.next_float() * 0.2 + 0.8;
            let ch = |shift: u32| ((col >> shift & 255) * 220 / 255) as f32 / 255.0 * variance;
            Some([ch(16), ch(8), ch(0)])
        }
        TintType::Custom => Some([16, 8, 0].map(|s| ((data.custom_tint >> s) & 255) as f32 / 255.0)),
        TintType::None => None,
    }
}

/// `TintType.WATER`: the biome's blended water colour mixed with the sky's (`tintMix`); none
/// with a shader pack when `shaderpackTint` asks so.
fn water_tint(game: &Game<'_>, cfg: &Config, b: [i32; 3]) -> Option<[f32; 3]> {
    if game.streaming.shader_pack && cfg.compat.shaderpack_tint {
        return None;
    }
    let color = water_color(game, b)?;
    let w = [(color >> 16 & 255) as f32, (color >> 8 & 255) as f32, (color & 255) as f32];
    let m = cfg.compat.tint_mix;
    Some([0, 1, 2].map(|i| (w[i] + m * (FOG_COLOR[i] - w[i])) / 255.0))
}

thread_local! {
    /// `BiomeColors.getAverageWaterColor` by block, kept for 80 ticks (the game time they were
    /// made at), as many particles ask at the same places.
    static WATER: std::cell::RefCell<(i64, HashMap<[i32; 3], Option<u32>>)> = std::cell::RefCell::new((i64::MIN, HashMap::new()));
}

/// The biome's blended water colour at a block (`BiomeColors.getAverageWaterColor`): the water
/// block's tint as the game blends it there.
fn water_color(game: &Game<'_>, b: [i32; 3]) -> Option<u32> {
    let now = game.world.game_time;
    if let Some(c) = WATER.with(|w| {
        let mut w = w.borrow_mut();
        if now.wrapping_sub(w.0) >= 80 || now < w.0 || w.1.len() > 65_536 {
            w.0 = now;
            w.1.clear();
        }
        w.1.get(&b).copied()
    }) {
        return c;
    }
    let water = game.pack.assets.registry.default_state("minecraft:water");
    let c = water.and_then(|water| {
        let chunks = &game.world.chunks;
        let (cx, cz) = (b[0] >> 4, b[2] >> 4);
        let region: [[Option<&horizon_client::chunk::Chunk>; 3]; 3] =
            std::array::from_fn(|dz| std::array::from_fn(|dx| chunks.get(&(cx + dx as i32 - 1, cz + dz as i32 - 1)).map(|c| &**c)));
        let biomes = match &game.world_biomes {
            Some(x) => x,
            None => game.res.biomes(),
        };
        game.pack.context.tables.terrain_particle_tint(water, b, &region, biomes, game.streaming.biome_zoom, game.streaming.biome_blend)
    });
    WATER.with(|w| w.borrow_mut().1.insert(b, c));
    c
}

/// The atlas name of a sprite id, or the missing sprite's.
fn sprite(game: &Game<'_>, id: &str) -> String {
    let name = horizon_client::hud::particlerain_atlas::sprite_name(&core::parse_id(id).unwrap_or_else(|| id.to_owned()));
    if game.gui.atlas.sprites.contains_key(&name) { name } else { horizon_client::hud::MISSING_SPRITE.to_owned() }
}

/// The rain sprites `CustomParticle` leaves untinted when `waterTint` is off.
fn rain_sprite(name: &str) -> bool {
    name.strip_prefix("particlerain:particle/").is_some_and(|n| {
        n.strip_prefix("rain_").or_else(|| n.strip_prefix("heavy_rain_")).is_some_and(|i| i.parse::<u32>().is_ok())
    })
}

// ---- spawning --------------------------------------------------------------------------------

/// A game particle Particle Rain asks for (`REGISTERED` style, or a streak's drip), made once the
/// tick's reading of the world is over.
struct Made {
    options: ParticleOptions,
    at: [f64; 3],
    /// The settings' index for the wind, and the spawn's wind.
    track: Option<(u32, [f64; 2])>,
}

impl Runtime {
    /// `ParticleStyle.spawn`.
    fn spawn(&mut self, lv: &Level<'_, '_>, at: usize, p: [f64; 3], made: &mut Vec<Made>) {
        let data = &lv.particles[at];
        match data.particle_style {
            ParticleStyle::Registered => {
                // `setPresetParticle`: a type that isn't one stays the cloud.
                let ty = ParticleType::from_name(&data.registered_particle_id).unwrap_or(ParticleType::Cloud);
                let m = wind_multiplier(lv.thundering, data);
                let wind = if m == 0.0 { [0.0; 2] } else { wind_at(lv.cfg, lv.game_time, p).map(|w| f64::from(w * m * 10.0)) };
                made.push(Made { options: ParticleOptions::simple(ty), at: p, track: Some((at as u32, wind)) });
            }
            ParticleStyle::BlockModel | ParticleStyle::Custom => {
                if let Some(d) = self.new_drop(lv, at, p) {
                    self.drops.push(d);
                }
            }
        }
    }

    /// `CustomParticle`'s (and `BlockDisplayParticle`'s) constructor.
    fn new_drop(&mut self, lv: &Level<'_, '_>, at: usize, p: [f64; 3]) -> Option<Drop> {
        let data = &lv.particles[at];
        let look = if data.particle_style == ParticleStyle::BlockModel {
            let name = core::parse_id(&data.block_id)?;
            Look::Block(lv.game.pack.assets.registry.default_state(&name)?)
        } else {
            Look::Quad
        };
        let sprite_id = if data.sprite_locations.is_empty() {
            "particlerain:new_custom_particle".to_owned()
        } else {
            let i = self.level_random.next_int_bound(data.sprite_locations.len() as i32).unwrap_or(0) as usize;
            data.sprite_locations[i].clone()
        };
        let mut random = LegacyRandom::new(self.seeds.next_long());
        let block = floor3(p);
        let mut d = Drop {
            id: data.id.clone(),
            at,
            look,
            pos: p,
            prev: p,
            vel: [0.0, if data.spawn_pos == SpawnPos::Sky { -f64::from(data.gravity) } else { f64::from(data.bounciness) }, 0.0],
            age: 0,
            lifetime: data.lifetime,
            gravity: data.gravity,
            removed: false,
            quad_size: data.size,
            o_quad_size: data.size,
            base_size: data.size,
            roll: 0.0,
            o_roll: 0.0,
            rotation_variation: 0.0,
            alpha: 0.0,
            rgb: [1.0; 3],
            sprite: sprite(lv.game, &sprite_id),
            block,
            o_block: block,
            base_temp: lv.base_temperature(block),
            collision_anim: false,
            progress: 1.0,
            o_progress: 1.0,
            speed: 0.0,
            distance: 0.0,
            edge_bounces: 0,
            yaw: 0.0,
            o_yaw: 0.0,
            random: LegacyRandom::new(0),
        };
        // WindManager.applySpawnWind.
        let m = wind_multiplier(lv.thundering, data);
        if m != 0.0 {
            let w = wind_at(lv.cfg, lv.game_time, p);
            d.vel[0] += f64::from(w[0] * m * 10.0);
            d.vel[2] += f64::from(w[1] * m * 10.0);
        }
        d.rotation_variation = if matches!(d.look, Look::Block(_)) {
            data.rotation_amount
        } else {
            data.rotation_amount * ((random.next_float() - 0.5) * 2.0)
        };
        d.random = random;
        if data.distance_scaling != 0.0 {
            d.quad_size = d.distance_size(data, self.fov);
        }
        // When waterTint is off, the rain keeps its own colour and no tint applies to it.
        if lv.cfg.compat.water_tint || !rain_sprite(&d.sprite) {
            if let Some(rgb) = tint(lv.game, lv.rain, data, lv.cfg, block, &mut self.level_random) {
                d.rgb = rgb;
            }
        }
        if data.rotation_type == RotationType::Horizontal {
            d.roll = HALF_PI * self.level_random.next_int_bound(4).unwrap_or(0) as f32;
        }
        d.o_roll = d.roll;
        d.yaw = (-(d.vel[0]) as f32).atan2(d.vel[2] as f32).to_degrees();
        d.o_yaw = d.yaw;
        Some(d)
    }

    /// `StreakParticle`'s constructor.
    fn new_streak(&mut self, lv: &Level<'_, '_>, at: usize, p: [f64; 3], face: Direction) -> Drop {
        let data = &lv.particles[at];
        let block = floor3(p);
        let rgb = if lv.cfg.compat.water_tint { water_tint(lv.game, lv.cfg, block).unwrap_or([1.0; 3]) } else { [0.2, 0.3, 1.0] };
        let roll = f32::from(match face {
            Direction::South => 0u8,
            Direction::West => 1,
            Direction::North => 2,
            Direction::East => 3,
            _ => 0,
        }) * HALF_PI;
        Drop {
            id: data.id.clone(),
            at,
            look: Look::Streak { face, list: data.block_list.clone() },
            pos: p,
            prev: p,
            vel: [0.0, -0.1, 0.0],
            age: 0,
            lifetime: lv.cfg.perf.particle_distance * 100,
            gravity: 0.0,
            removed: false,
            quad_size: lv.cfg.streak.size,
            o_quad_size: lv.cfg.streak.size,
            base_size: lv.cfg.streak.size,
            roll,
            o_roll: roll,
            rotation_variation: 0.0,
            alpha: lv.cfg.streak.opacity,
            rgb,
            sprite: sprite(lv.game, "particlerain:streak"),
            block,
            o_block: block,
            base_temp: 0.0,
            collision_anim: false,
            progress: 1.0,
            o_progress: 1.0,
            speed: 0.0,
            distance: 0.0,
            edge_bounces: 0,
            yaw: 0.0,
            o_yaw: 0.0,
            random: LegacyRandom::new(self.seeds.next_long()),
        }
    }

    /// `ParticleSpawner.getHeight`: the MOTION_BLOCKING height, or with blocks weather ignores
    /// `calculateHeight`, kept for 80 ticks.
    fn height(&mut self, lv: &Level<'_, '_>, x: i32, z: i32) -> i32 {
        if lv.cfg.compat.weather_ignore_blocks.entries.is_empty() {
            return lv.motion_blocking(x, z);
        }
        if lv.game_time != self.last_tick {
            if lv.game_time % 80 == 0 {
                self.heights.clear();
            }
            self.last_tick = lv.game_time;
        }
        if let Some(&y) = self.heights.get(&(x, z)) {
            return y;
        }
        let y = lv.calculate_height(x, z);
        self.heights.insert((x, z), y);
        y
    }

    /// `ParticleSpawner.tickBlockFX` for one block the ambient tick sampled.
    fn block_fx(&mut self, lv: &Level<'_, '_>, source: [i32; 3], made: &mut Vec<Made>) {
        if self.spawn_attempts_until_block_fx_idle <= 0 && self.level_random.next_float() < 0.9 {
            return;
        }
        self.spawn_attempts_until_block_fx_idle -= 1;
        let state = lv.state(source);
        if !lv.no_collision(state) && !lv.ignored(state) {
            return;
        }
        for at in 0..lv.particles.len() {
            let data = &lv.particles[at];
            if !data.enabled || !data.weather.is_current(lv.rain, lv.thunder, self.after_weather_ticks_left > 0) {
                continue;
            }
            let face = match data.spawn_pos {
                SpawnPos::BlockSides => [Direction::North, Direction::East, Direction::South, Direction::West]
                    [self.level_random.next_int_bound(4).unwrap_or(0) as usize],
                SpawnPos::BlockBottom => Direction::Down,
                SpawnPos::BlockTop => Direction::Up,
                _ => continue,
            };
            let n = face.opposite().normal();
            let pos = [source[0] + n[0], source[1] + n[1], source[2] + n[2]];
            let block_state = lv.state(pos);
            let shape = lv.shape(block_state);
            let fluid = lv.fluid(block_state);
            let full = lv.facts(block_state).full_collision;
            if (shape.is_none() && !full && !fluid) || lv.ignored(state) {
                continue;
            }
            if !data.precipitation.contains(&lv.precipitation(source))
                || data.density <= self.level_random.next_float()
                || !lv.biome_ok(&data.biome_list, source)
                || !lv.block_ok(&data.block_list, block_state)
            {
                continue;
            }
            if data.needs_sky_access && source[1] < self.height(lv, source[0], source[2]) {
                continue;
            }
            // A point on the face (`VoxelShape.max`/`min` along its axis at two random places).
            let (p1, p2) = (self.level_random.next_float(), self.level_random.next_float());
            let axis = match face {
                Direction::Down | Direction::Up => horizon_core::geometry::Axis::Y,
                Direction::North | Direction::South => horizon_core::geometry::Axis::Z,
                Direction::West | Direction::East => horizon_core::geometry::Axis::X,
            };
            let a = axis as usize;
            let positive = matches!(face, Direction::Up | Direction::South | Direction::East);
            let edge = match (shape, positive) {
                (Some(s), true) => s.max_at(axis, f64::from(p1), f64::from(p2)),
                (Some(s), false) => s.min_at(axis, f64::from(p1), f64::from(p2)),
                (None, true) if full => 1.0,
                (None, false) if full => 0.0,
                (None, true) => f64::NEG_INFINITY,
                (None, false) => f64::INFINITY,
            };
            let edge = if face == Direction::Up { edge.max(lv.fluid_height(pos)) } else { edge };
            if !edge.is_finite() {
                continue;
            }
            let edge = if positive { edge + 0.01 } else { edge - 0.01 };
            let mut local = [0.0f64; 3];
            local[a] = edge;
            local[(a + 1) % 3] = f64::from(p1);
            local[(a + 2) % 3] = f64::from(p2);
            let p = [f64::from(pos[0]) + local[0], f64::from(pos[1]) + local[1], f64::from(pos[2]) + local[2]];
            if data.particle_style == ParticleStyle::Registered && data.registered_particle_id == "particlerain:streak" {
                let d = self.new_streak(lv, at, p, face);
                self.drops.push(d);
            } else {
                self.spawn(lv, at, p, made);
            }
            self.spawn_attempts_until_block_fx_idle = 10000;
        }
    }

    /// `ParticleSpawner.getSkyParticleDensity`.
    fn sky_density(&mut self, cfg: &Config, thunder: f32, speed: f32) -> i32 {
        let mut density = cfg.perf.particle_amount / 25;
        if self.ticks_until_sky_fx_idle <= 0 {
            density = 4;
        } else {
            self.ticks_until_sky_fx_idle -= 1;
            let storm = (density as f32 * cfg.perf.particle_storm_density) as i32;
            density += (thunder * (storm - density) as f32).floor() as i32;
            density *= 1 + speed as i32 * 2;
        }
        density
    }

    /// `ParticleSpawner.tickSkyFX`: points on a sphere round the camera, those above the ground.
    fn sky_fx(&mut self, lv: &Level<'_, '_>, made: &mut Vec<Made>) {
        let speed = (lv.camera_velocity.iter().map(|v| v * v).sum::<f64>()).sqrt() as f32;
        let density = self.sky_density(lv.cfg, lv.thunder, speed);
        let distance = lv.cfg.perf.particle_distance as f32;
        for _ in 0..density {
            let height = if speed < 0.8 {
                let (a, b) = (self.random.next_float(), self.random.next_float());
                let h = -(a * a - b * b).abs() + 1.0;
                h * (0.4 + 0.6)
            } else {
                self.random.next_float()
            };
            let theta = std::f32::consts::TAU * self.random.next_float();
            let phi = (2.0 * height - 1.0).acos();
            let x = distance * msin(phi) * mcos(theta) + lv.camera[0] as f32;
            let mut y = distance * mcos(phi) + lv.camera[1] as f32;
            let z = distance * msin(phi) * msin(theta) + lv.camera[2] as f32;
            let mut pos = [x.floor() as i32, y.floor() as i32, z.floor() as i32];
            if !lv.loaded(pos) {
                continue;
            }
            if lv.cfg.compat.do_spawn_height_limit {
                let cloud = if lv.cfg.compat.spawn_height_limit == 0 { cloud_height() } else { lv.cfg.compat.spawn_height_limit };
                if cloud != 0 && y > cloud as f32 {
                    y = cloud as f32;
                    pos[1] = cloud;
                }
            }
            let top = self.height(lv, pos[0], pos[2]);
            let below = [x.floor() as i32, top - 1, z.floor() as i32];
            if top >= pos[1] {
                continue;
            }
            let precipitation = lv.precipitation(if lv.cfg.compat.use_heightmap_temp { below } else { pos });
            let chance = self.random.next_float();
            let below_state = lv.state(below);
            for at in 0..lv.particles.len() {
                let data = &lv.particles[at];
                if data.enabled
                    && data.spawn_pos == SpawnPos::Sky
                    && data.weather.is_current(lv.rain, lv.thunder, self.after_weather_ticks_left > 0)
                    && data.precipitation.contains(&precipitation)
                    && data.density > chance
                    && lv.biome_ok(&data.biome_list, pos)
                    && lv.block_ok(&data.block_list, below_state)
                {
                    self.spawn(lv, at, [f64::from(x), f64::from(y), f64::from(z)], made);
                    self.ticks_until_sky_fx_idle = 100;
                }
            }
        }
    }

    /// `ParticleSpawner.tickSurfaceFX`: places on the ground round the camera.
    fn surface_fx(&mut self, lv: &Level<'_, '_>, made: &mut Vec<Made>) {
        let density = if self.ticks_until_surface_fx_idle <= 0 {
            1
        } else {
            self.ticks_until_surface_fx_idle -= 1;
            lv.cfg.perf.particle_amount / 200
        };
        let range = f64::from(lv.cfg.perf.surface_range);
        for _ in 0..density {
            let x = lv.camera[0] + range * (self.random.next_double() - self.random.next_double());
            let z = lv.camera[2] + range * (self.random.next_double() - self.random.next_double());
            let y = self.height(lv, x.floor() as i32, z.floor() as i32);
            let below = [x.floor() as i32, y - 1, z.floor() as i32];
            if !lv.loaded(below) {
                continue;
            }
            let state = lv.state(below);
            let precipitation = lv.precipitation(below);
            let chance = self.random.next_float();
            for at in 0..lv.particles.len() {
                let data = &lv.particles[at];
                if data.enabled
                    && data.density > chance
                    && data.spawn_pos == SpawnPos::WorldSurface
                    && data.weather.is_current(lv.rain, lv.thunder, self.after_weather_ticks_left > 0)
                    && data.precipitation.contains(&precipitation)
                    && lv.biome_ok(&data.biome_list, below)
                    && lv.block_ok(&data.block_list, state)
                {
                    let py = f64::from(y) + self.random.next_double() + 0.1;
                    self.spawn(lv, at, [x, py, z], made);
                    self.ticks_until_surface_fx_idle = 100;
                }
            }
        }
    }
}

/// `VersionUtil.getCloudHeight`: the overworld's cloud height.
fn cloud_height() -> i32 {
    horizon_client::clouds::CLOUD_HEIGHT as i32
}

// ---- a particle's tick -------------------------------------------------------------------------

impl Drop {
    fn data<'p>(&mut self, particles: &'p [ParticleData]) -> Option<&'p ParticleData> {
        if particles.get(self.at).is_none_or(|d| d.id != self.id) {
            self.at = particles.iter().position(|d| d.id == self.id)?;
        }
        particles.get(self.at)
    }

    fn render_distance(data: &ParticleData, cfg: &Config) -> f32 {
        if data.spawn_pos == SpawnPos::WorldSurface { cfg.perf.surface_range as f32 } else { cfg.perf.particle_distance as f32 }
    }

    /// `getDistanceSize`.
    fn distance_size(&self, data: &ParticleData, fov: f32) -> f32 {
        let scaled = self.base_size * self.distance * data.distance_scaling;
        let size = self.base_size + scaled;
        if data.distance_scaling != 1.0 {
            return size;
        }
        size * (fov.min(70.0) / 70.0)
    }

    /// `Particle.tick` with `WeatherParticle.move` (no collision).
    fn base_tick(&mut self) {
        self.prev = self.pos;
        let age = self.age;
        self.age += 1;
        if age >= self.lifetime {
            self.removed = true;
        } else {
            self.vel[1] -= 0.04 * f64::from(self.gravity);
            for i in 0..3 {
                self.pos[i] += self.vel[i];
            }
            let friction = f64::from(0.98f32);
            for v in &mut self.vel {
                *v *= friction;
            }
        }
    }

    /// `getPitchAngleToCamera`.
    fn pitch_to_camera(&self, camera: [f64; 3]) -> f32 {
        let ox = (self.pos[0] - camera[0]) as f32;
        let oy = (self.pos[1] - camera[1]) as f32;
        let oz = (self.pos[2] - camera[2]) as f32;
        oy.atan2((ox * ox + oz * oz).sqrt()).abs()
    }

    /// `CustomParticle.tick` (and `BlockDisplayParticle.tick`).
    fn tick_custom(&mut self, lv: &Level<'_, '_>, rt: &mut Runtime, data: &ParticleData) {
        self.base_tick();
        // WeatherParticle.tick.
        self.o_quad_size = self.quad_size;
        self.block = floor3(self.pos);
        if self.collision_anim {
            self.tick_collision_anim(data);
        }
        self.o_quad_size = self.quad_size;
        let c = lv.camera;
        self.distance = ((self.pos[0] - c[0]).powi(2) + (self.pos[1] - c[1]).powi(2) + (self.pos[2] - c[2]).powi(2)).sqrt() as f32;
        if self.distance > Self::render_distance(data, lv.cfg) + 0.5 {
            self.removed = true;
        }
        if self.block != self.o_block {
            self.on_position_update(lv, data);
            self.o_block = self.block;
        }
        self.speed = (self.vel.iter().map(|v| v * v).sum::<f64>()).sqrt() as f32;
        self.tick_fading(lv, data);
        if data.distance_scaling != 0.0 && !self.collision_anim {
            self.quad_size = self.distance_size(data, rt.fov);
        }
        if data.rotation_amount != 0.0 {
            self.o_roll = self.roll;
            self.roll += self.rotation_variation * self.speed;
        }
        if data.animate_sprites && data.sprite_locations.len() > 1 {
            let i = (self.age - 1).rem_euclid(data.sprite_locations.len() as i32 - 1) as usize;
            self.sprite = sprite(lv.game, &data.sprite_locations[i]);
        }
        // The fog's colour follows the weather while it changes.
        if data.tint_type == TintType::Fog && lv.rain > 0.0 && lv.rain < 1.0
            && let Some(rgb) = tint(lv.game, lv.rain, data, lv.cfg, self.block, &mut rt.level_random)
        {
            self.rgb = rgb;
        }
        // WindManager.applyWind.
        let m = wind_multiplier(lv.thundering, data);
        if m != 0.0 {
            let w = wind_at(lv.cfg, lv.game_time, self.pos);
            self.vel[0] += f64::from(w[0] * m);
            self.vel[2] += f64::from(w[1] * m);
        }
        self.tick_collisions(lv, data);
        if matches!(self.look, Look::Block(_)) {
            // BlockDisplayParticle.updateDisplay: the entity faces where it goes.
            self.o_yaw = self.yaw;
            self.yaw = (-(self.vel[0]) as f32).atan2(self.vel[2] as f32).to_degrees();
        }
    }

    /// `CustomParticle.tickCollisionAnim`.
    fn tick_collision_anim(&mut self, data: &ParticleData) {
        self.o_progress = self.progress;
        self.progress -= self.speed;
        if data.rotation_type != RotationType::RelativeVelocity {
            self.quad_size = (self.quad_size - self.speed).max(0.0);
        }
        if self.o_progress <= 0.0 {
            self.removed = true;
        }
    }

    /// `tickFading`.
    fn tick_fading(&mut self, lv: &Level<'_, '_>, data: &ParticleData) {
        let half = (self.lifetime / 2) as f32;
        match data.fade_type {
            FadeType::InAndOut => {
                let half_life = self.lifetime / 2;
                let t = if self.age < half_life { self.age as f32 / half } else { (self.lifetime - self.age) as f32 / half };
                self.alpha = t * data.opacity;
            }
            FadeType::Out => self.alpha = (self.lifetime - self.age) as f32 / half * data.opacity,
            FadeType::Distance => {
                let falloff = 3.0;
                let fd = Self::render_distance(data, lv.cfg) / falloff;
                let t = ((self.distance + fd) / fd - falloff).clamp(0.0, 1.0);
                self.alpha = data.opacity + t * (0.0 - data.opacity);
            }
        }
        const QUARTER_PI: f32 = 0.785_398_2;
        if data.rotation_type == RotationType::Vertical {
            let angle = self.pitch_to_camera(lv.camera);
            self.alpha = self.alpha.min(-(angle.min(QUARTER_PI) / QUARTER_PI) + 1.0);
        } else if data.rotation_type == RotationType::Horizontal {
            let angle = self.pitch_to_camera(lv.camera);
            self.alpha = self.alpha.min(angle.min(0.4) / 0.4);
        }
    }

    /// `onPositionUpdate`: into another climate it ends; inside a full block it goes.
    fn on_position_update(&mut self, lv: &Level<'_, '_>, data: &ParticleData) {
        if !lv.cfg.compat.cross_biome_border
            && lv.base_temperature(self.block) != self.base_temp
            && !data.precipitation.contains(&lv.precipitation(self.block))
        {
            self.collision_anim = true;
            return;
        }
        let s = lv.state(self.block);
        let ignores = lv.ignored(s);
        if (!ignores || lv.fluid(s)) && lv.facts(s).full_collision {
            self.removed = true;
        }
    }

    /// `tickCollisions`: a ray along the motion, as long as the particle and its step.
    fn tick_collisions(&mut self, lv: &Level<'_, '_>, data: &ParticleData) {
        let d = self.vel;
        let len = (d.iter().map(|v| v * v).sum::<f64>()).sqrt();
        let length = f64::from(self.quad_size) + len;
        let n = if len < 1.0e-5 { [0.0; 3] } else { d.map(|v| v / len) };
        let edge = [n[0] * length + self.pos[0], n[1] * length + self.pos[1], n[2] * length + self.pos[2]];
        if let Some((block, face)) = lv.clip(self.pos, edge)
            && !self.collision_anim
        {
            self.on_collision(lv, data, block, face);
        }
    }

    /// `onCollision`: a bouncy particle bounces off a solid face; otherwise it ends.
    fn on_collision(&mut self, lv: &Level<'_, '_>, data: &ParticleData, block: [i32; 3], face: Direction) {
        let s = lv.state(block);
        if lv.ignored(s) {
            return;
        }
        let f = lv.facts(s);
        let source = f.fluid != horizon_client::blocks::Fluid::None && f.fluid_level == 8 && !f.fluid_falling;
        if !source && data.bounciness != 0.0 {
            let normal = face.normal().map(|v| v as f32);
            let bounciness = data.bounciness * self.speed * 6.0;
            if normal[1] == 0.0 {
                let bounces = self.edge_bounces;
                self.edge_bounces += 1;
                if bounces >= 3 {
                    self.collision_anim = true;
                }
                self.vel[1] += f64::from(0.5 * bounciness);
            }
            for (v, n) in self.vel.iter_mut().zip(normal) {
                *v += f64::from(n * bounciness);
            }
        } else {
            self.collision_anim = true;
        }
    }

    /// `StreakParticle.tick`: down the wall, a drip where the wall ends.
    fn tick_streak(&mut self, lv: &Level<'_, '_>, made: &mut Vec<Made>) {
        let Look::Streak { face, list } = &self.look else { return };
        let (face, list) = (*face, list.clone());
        self.base_tick();
        self.o_quad_size = self.quad_size;
        self.block = floor3(self.pos);
        if self.collision_anim {
            // StreakParticle.tickCollisionAnim.
            self.gravity = 0.0;
            self.vel[1] = 0.0;
            self.alpha -= 0.1;
            if self.alpha <= 0.0 {
                self.removed = true;
            }
        }
        if self.age % 10 == 0 {
            self.gravity = if self.random.next_boolean() { self.random.next_float() / 10.0 } else { 0.0 };
        }
        let n = face.opposite().normal();
        let end = [self.pos[0] + f64::from(n[0]) * 0.06, self.pos[1] + f64::from(n[1]) * 0.06, self.pos[2] + f64::from(n[2]) * 0.06];
        match lv.clip(self.pos, end) {
            None => {
                if !self.collision_anim {
                    made.push(Made {
                        options: ParticleOptions::simple(ParticleType::DrippingWater),
                        at: [self.pos[0], self.pos[1] - 0.05, self.pos[2]],
                        track: None,
                    });
                    self.collision_anim = true;
                }
            }
            Some((behind, _)) => {
                let s = lv.state(behind);
                if !lv.block_ok(&list, s) || lv.fluid(lv.state(self.block)) {
                    self.collision_anim = true;
                }
            }
        }
    }
}

// ---- the tick ----------------------------------------------------------------------------------

/// Once a client tick, after the game's particle tick: the game's weather effects as Particle
/// Rain changes them, the mod's particles' tick, the spawner, the wind on the game particles it
/// spawned, and the water's colour on the game's water particles. `weather` is the Horizon
/// Weather option as it was before the game's tick.
pub(super) fn tick(game: &mut Game<'_>, weather: bool) {
    let Some(eye) = game.eye_position() else { return };
    let (rain, thunder) = game.weather_levels(1.0);
    let thundering = game.can_have_weather() && thunder > 0.9;
    let camera_velocity = game.player.as_ref().map_or([0.0; 3], |p| [p.velocity.x, p.velocity.y, p.velocity.z]);
    let mut made = Vec::new();
    let tracked = glue::with(|st| {
        let tags = st.tags.get_or_insert_with(|| core::Tags::load(&game.vanilla_root()));
        let lv = Level {
            game,
            cfg: &st.config,
            particles: &st.particles,
            tags: &*tags,
            rain,
            thunder,
            thundering,
            camera: eye,
            camera_velocity,
            game_time: game.world.game_time,
        };
        with(|rt| {
            // WeatherDataMixin: the rain stopping starts up to five minutes of "after weather".
            let raining = game.world.weather.raining;
            if rt.raining.is_some_and(|r| r != raining) {
                rt.after_weather_ticks_left = if raining { 0 } else { rt.level_random.next_int_bound(6000).unwrap_or(0) };
            }
            rt.raining = Some(raining);
            if rain > 0.0 && weather {
                weather_effects(&lv, rt);
            }
            // The ambient tick's samples (ClientLevel.doAnimateTick: 667 times at 16 and 32 blocks).
            let center = floor3(eye);
            for _ in 0..667 {
                for r in [16, 32] {
                    let mut pick = || rt.level_random.next_int_bound(r).unwrap_or(0) - rt.level_random.next_int_bound(r).unwrap_or(0);
                    let p = [center[0] + pick(), center[1] + pick(), center[2] + pick()];
                    if lv.world().loaded(p[0], p[2]) {
                        rt.block_fx(&lv, p, &mut made);
                    }
                }
            }
            // The particles' tick (ParticleEngine.tick).
            let mut drops = std::mem::take(&mut rt.drops);
            for d in &mut drops {
                if matches!(d.look, Look::Streak { .. }) {
                    d.tick_streak(&lv, &mut made);
                    continue;
                }
                match d.data(lv.particles) {
                    Some(data) => d.tick_custom(&lv, rt, data),
                    None => d.removed = true,
                }
            }
            drops.retain(|d| !d.removed);
            drops.append(&mut rt.drops);
            rt.drops = drops;
            // ParticleSpawner.tick (END_CLIENT_TICK).
            if (rt.drops.len() as i64) < i64::from(lv.cfg.perf.particle_amount) {
                rt.sky_fx(&lv, &mut made);
                rt.surface_fx(&lv, &mut made);
                if rt.after_weather_ticks_left > 0 {
                    rt.after_weather_ticks_left -= 1;
                }
            }
        });
        // The wind of each tracked particle's settings.
        st.particles.iter().map(|d| wind_multiplier(thundering, d)).collect::<Vec<_>>()
    });
    apply_weather_effects(game);
    for m in made {
        match m.track {
            Some((tag, wind)) => {
                pp::create_tracked(&mut game.particles, &m.options, m.at, tag, wind);
            }
            None => game.particles.create(&m.options, m.at, [0.0; 3]),
        }
    }
    // WindManager.tick.
    let cfg = glue::with(|st| st.config.clone());
    let game_time = game.world.game_time;
    pp::tracked(&mut game.particles, |p, tag| {
        let m = tracked.get(tag as usize).copied().unwrap_or(0.0);
        if m != 0.0 {
            let w = wind_at(&cfg, game_time, pp::position(p));
            p.velocity[0] += f64::from(w[0] * m);
            p.velocity[2] += f64::from(w[1] * m);
        }
    });
    // WaterDropParticleMixin, DripParticleMixin.
    if cfg.compat.water_tint {
        let mut tints: Vec<([f64; 3], Option<[f32; 3]>)> = Vec::new();
        let mut want = Vec::new();
        pp::fresh_water(&mut game.particles, |p| {
            want.push(p);
            None
        }, || None);
        for p in want {
            tints.push((p, water_tint(game, &cfg, floor3(p))));
        }
        let mut random = with(|rt| LegacyRandom::new(rt.seeds.next_long()));
        let mut i = 0;
        pp::fresh_water(
            &mut game.particles,
            |_| {
                let t = tints.get(i).and_then(|t| t.1);
                i += 1;
                t
            },
            || Some(format!("particlerain:particle/splash_{}", random.next_int_bound(4).unwrap_or(0))),
        );
    }
}

/// `ClientLevel.tickWeatherEffects` with `ClientLevelMixin`: the rain's place within the weather
/// radius found everywhere (not only where it rains) on the mod's heights, splashes only with
/// "Do default splashing", the snow and sandstorm sounds where it snows or is dry, and the rain
/// sound at the mod's volume, only where it rains.
fn weather_effects(lv: &Level<'_, '_>, rt: &mut Runtime) {
    let game = lv.game;
    let cfg = lv.cfg;
    let block = floor3(lv.camera);
    let radius = game.settings.weather_radius;
    let diameter = 2 * radius + 1;
    let level = if cfg.compat.do_default_splashing { game.settings.particles } else { 2 };
    let count = (0.225 * (diameter * diameter) as f32 * lv.rain * lv.rain) as i32 / if level == 1 { 2 } else { 1 };
    let mut random = LegacyRandom::new(game.world.game_time.wrapping_mul(312_987_231));
    let mut rain_position = None;
    let mut splashes = Vec::new();
    for _ in 0..count {
        let x = block[0] + random.next_int_bound(diameter).unwrap_or(0) - radius;
        let z = block[2] + random.next_int_bound(diameter).unwrap_or(0) - radius;
        let top = rt.height(lv, x, z);
        if !(top > game.world.bottom_y() && (block[1] - 10..=block[1] + 10).contains(&top)) {
            continue;
        }
        let below = top - 1;
        rain_position = Some([x, below, z]);
        if level >= 2 {
            break;
        }
        let (bx, bz) = (random.next_double(), random.next_double());
        let state = lv.state([x, below, z]);
        let f = lv.facts(state);
        let block_top = match lv.shape(state) {
            Some(shape) => shape.max_at(horizon_core::geometry::Axis::Y, bx, bz),
            None => 0.0,
        };
        let fluid_top = if f.fluid == horizon_client::blocks::Fluid::None { 0.0 } else { f64::from(f.fluid_level) / 9.0 };
        let smoke = f.fluid == horizon_client::blocks::Fluid::Lava
            || &*f.name == "minecraft:magma_block"
            || (f.name.ends_with("campfire") && game.res.animate().is_lit_campfire(state.0));
        let ty = if smoke { ParticleType::Smoke } else { ParticleType::Rain };
        splashes.push((ty, [f64::from(x) + bx, f64::from(below) + block_top.max(fluid_top), f64::from(z) + bz]));
    }
    let mut sounds: Vec<(&'static str, [f64; 3], f32, f32, bool)> = Vec::new();
    if let Some(at) = rain_position {
        let waited = game.rain_sound_time;
        let reset = random.next_int_bound(3).unwrap_or(0) < waited;
        RAIN_SOUND.with(|r| r.set(Some(if reset { 0 } else { waited + 1 })));
        if reset {
            // ParticleRain.doAdditionalWeatherSounds.
            let rain_pos = [at[0], rt.height(lv, at[0], at[2]), at[2]];
            let above_clouds = cfg.compat.do_spawn_height_limit && {
                let cloud = if cfg.compat.spawn_height_limit == 0 { cloud_height() } else { cfg.compat.spawn_height_limit };
                rain_pos[1] > cloud
            };
            if !above_clouds {
                let roof = rt.height(lv, block[0], block[2]);
                let above = rain_pos[1] > block[1] + 1 && roof > lv.camera[1].floor() as i32;
                let precipitation = lv.precipitation(rain_pos);
                let p = rain_pos.map(|c| f64::from(c) + 0.5);
                let name = lv.biome_name(rain_pos);
                if precipitation == Precipitation::Snow && cfg.sound.snow_volume > 0.0 {
                    sounds.push((if above { "snow.above" } else { "snow" }, p, cfg.sound.snow_volume, if above { 0.5 } else { 1.0 }, false));
                } else if lv.tags.biome_in(&name, "c:in_overworld")
                    && precipitation == Precipitation::None
                    && lv.base_temperature(rain_pos) > 0.25
                    && cfg.sound.wind_volume > 0.0
                {
                    sounds.push((if above { "sandstorm.above" } else { "sandstorm" }, p, cfg.sound.wind_volume, if above { 0.5 } else { 1.0 }, false));
                }
                if cfg.sound.rain_volume != 0.0 && precipitation == Precipitation::Rain {
                    let (name, pitch) = if at[1] > block[1] + 1 && roof > block[1] { ("minecraft:weather.rain.above", 0.5) } else { ("minecraft:weather.rain", 1.0) };
                    sounds.push((name, at.map(|c| f64::from(c) + 0.5), cfg.sound.rain_volume, pitch, true));
                }
            }
        }
    }
    WEATHER_OUT.with(|w| *w.borrow_mut() = (splashes, sounds));
}

thread_local! {
    /// What `weather_effects` asks of the game (it only reads it): the splashes and the sounds.
    #[allow(clippy::type_complexity)]
    static WEATHER_OUT: std::cell::RefCell<(Vec<(ParticleType, [f64; 3])>, Vec<(&'static str, [f64; 3], f32, f32, bool)>)> =
        const { std::cell::RefCell::new((Vec::new(), Vec::new())) };
    static RAIN_SOUND: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
}

/// The splashes and sounds `weather_effects` asked for, into the game.
fn apply_weather_effects(game: &mut Game<'_>) {
    if let Some(t) = RAIN_SOUND.with(std::cell::Cell::take) {
        game.rain_sound_time = t;
    }
    let (splashes, sounds) = WEATHER_OUT.with(|w| std::mem::take(&mut *w.borrow_mut()));
    for (ty, p) in splashes {
        game.particles.add(&ParticleOptions::simple(ty), p, [0.0; 3]);
    }
    for (name, at, volume, pitch, vanilla) in sounds {
        if vanilla {
            // playLocalSound: guests make their own rain, so a host must not capture it for them.
            game.audio.play_unshared(name, Some(at), volume, pitch);
        } else {
            horizon_client::audio::particlerain_sound::play(&mut game.audio, SOUNDS, name, at, volume, pitch);
        }
    }
}

// ---- the frame ---------------------------------------------------------------------------------

/// `ARGB.colorFromFloat` (each channel `floor(v * 255)`, its low byte) as the vertex colour.
fn color(alpha: f32, rgb: [f32; 3]) -> u32 {
    let c = |v: f32| ((v * 255.0).floor() as i32 & 0xFF) as u32;
    c(alpha) << 24 | c(rgb[2]) << 16 | c(rgb[1]) << 8 | c(rgb[0])
}

/// `WeatherParticle.turnBackfaceFlipways`: the quad turned round when its front faces away.
fn flip(q: glam::Quat, offset: glam::Vec3) -> glam::Quat {
    let normal = (q * glam::Vec3::Z).normalize();
    if normal.dot(offset) > 0.0 { q * glam::Quat::from_rotation_y(std::f32::consts::PI) } else { q }
}

/// The frame's quads of the mod's particles, handed to the particle pass
/// (`SingleQuadParticle.extractRotatedQuad` per particle, block models for block particles).
pub(super) fn frame(game: &Game<'_>, camera: glam::DVec3, partial: f32, fov: f32) {
    if game.window.is_none() || !game.native_on(glue::ID) {
        pp::set_frame(Vec::new());
        return;
    }
    let particles = glue::with(|st| st.particles.clone());
    let player_velocity = game.player.as_ref().map_or(glam::Vec3::ZERO, |p| glam::Vec3::new(p.velocity.x as f32, p.velocity.y as f32, p.velocity.z as f32));
    let base = glam::Quat::from_rotation_y(std::f32::consts::PI - game.yaw.to_radians()) * glam::Quat::from_rotation_x(-game.pitch.to_radians());
    let mut out: Vec<horizon_client::mesher::Vertex> = Vec::new();
    with(|rt| {
        rt.fov = fov;
        for d in &mut rt.drops {
            let a = f64::from(partial);
            let world = [0, 1, 2].map(|i| d.prev[i] + (d.pos[i] - d.prev[i]) * a);
            let offset = (glam::DVec3::from_array(world) - camera).as_vec3();
            let b = floor3(d.pos);
            let packed = game.world.light(b[0], b[1], b[2]);
            let light = (u32::from(packed >> 4) * 16) | ((u32::from(packed & 15) * 16) << 8);
            let size = d.o_quad_size + (d.quad_size - d.o_quad_size) * partial;
            let roll = d.o_roll + (d.roll - d.o_roll) * partial;
            let data = d.data(&particles);
            if let Look::Block(state) = d.look {
                // BlockDisplayParticle: the display entity's yaw, then its transformation.
                let mut dy = d.yaw - d.o_yaw;
                dy = (dy + 180.0).rem_euclid(360.0) - 180.0;
                let yaw = d.o_yaw + dy * partial;
                let t = glam::Vec3::new(0.0, -mcos(roll + 0.8) * 0.7, -mcos(roll + 5.5) * 0.7);
                let m = glam::Mat4::from_translation(offset)
                    * glam::Mat4::from_quat(glam::Quat::from_rotation_y(-yaw.to_radians()))
                    * glam::Mat4::from_translation(t)
                    * glam::Mat4::from_scale(glam::Vec3::splat(size))
                    * glam::Mat4::from_quat(glam::Quat::from_axis_angle(glam::Vec3::X, roll));
                game.block_model_vertices(state, 0, m, light, true, &mut out);
                continue;
            }
            let (rotation, center) = match (&d.look, data.map(|x| x.rotation_type)) {
                (Look::Streak { .. }, _) => {
                    let q = flip(glam::Quat::from_axis_angle(glam::Vec3::Y, d.roll), offset);
                    (q, offset + glam::Vec3::new(0.0, d.quad_size, 0.0))
                }
                (_, Some(RotationType::Vertical)) => {
                    let q = glam::Quat::from_rotation_y(offset.x.atan2(offset.z)) * glam::Quat::from_rotation_z(roll);
                    (flip(q, offset), offset)
                }
                (_, Some(RotationType::Horizontal)) => {
                    let mut q = glam::Quat::from_axis_angle(glam::Vec3::NEG_X, HALF_PI);
                    if d.roll != 0.0 {
                        q *= glam::Quat::from_rotation_z(roll);
                    }
                    (flip(q, offset), offset)
                }
                (_, Some(r @ (RotationType::RelativeVelocity | RotationType::WorldVelocity))) => {
                    let mut motion = glam::Vec3::new(d.vel[0] as f32, d.vel[1] as f32, d.vel[2] as f32);
                    if r == RotationType::RelativeVelocity {
                        motion -= player_velocity;
                    }
                    // velocityAngle, velocityAxis.
                    let l = motion.length_squared();
                    let direction = if l.is_finite() && l > 1.0e-20 { motion.normalize() } else { glam::Vec3::NEG_Y };
                    let angle = direction.y.clamp(-1.0, 1.0).acos();
                    let axis = glam::Vec3::new(-motion.z, 0.0, motion.x);
                    let la = axis.length_squared();
                    let axis = if la.is_finite() && la > 1.0e-20 { axis.normalize() } else { glam::Vec3::X };
                    let turned = glam::Quat::from_axis_angle(axis, angle) * offset;
                    let q = glam::Quat::from_axis_angle(axis, -angle) * glam::Quat::from_rotation_y(turned.x.atan2(turned.z) + std::f32::consts::PI);
                    (q, offset)
                }
                _ => {
                    let q = if d.roll != 0.0 { base * glam::Quat::from_rotation_z(roll) } else { base };
                    (q, offset)
                }
            };
            let render_type = data.map_or(RenderType::Translucent, |x| x.render_type);
            let layer = if render_type == RenderType::Opaque && !matches!(d.look, Look::Streak { .. }) {
                0
            } else {
                horizon_client::gfx::TRANSLUCENT_PARTICLE_OVERLAY
            };
            let r = game
                .gui
                .atlas
                .sprites
                .get(&d.sprite)
                .or_else(|| game.gui.atlas.sprites.get(horizon_client::hud::MISSING_SPRITE))
                .copied();
            let Some(r) = r else { continue };
            let col = color(d.alpha, d.rgb);
            for (n, uv) in [((1.0, -1.0), [r.u1, r.v1]), ((1.0, 1.0), [r.u1, r.v0]), ((-1.0, 1.0), [r.u0, r.v0]), ((-1.0, -1.0), [r.u0, r.v1])] {
                let v = center + rotation * glam::Vec3::new(n.0, n.1, 0.0) * size;
                out.push(horizon_client::mesher::Vertex {
                    normal: [0; 4],
                    pos: v.to_array(),
                    uv,
                    layer,
                    color: col,
                    light,
                    glint_uv: [0.0, 0.0],
                });
            }
        }
    });
    pp::set_frame(out);
}
