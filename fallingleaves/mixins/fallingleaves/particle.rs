// Falling Leaves' particle (`FallingLeafParticle`), inside Horizon's particle
// engine: a billboard that falls, spins, drifts with the wind, floats on water
// and lies where it lands, then fades. Rewritten from Falling Leaves by
// RandomMcSomethin (MIT, https://github.com/RandomMcSomethin/fallingleaves).
//
// Spliced into the engine's own module (`horizon_client::particles`), so it can
// set what the public API keeps private: the roll and the sprite. Its own
// textures sit in the atlas slot for a mod's image (`horizon/modimage`), in the
// strip `TILE_ROW` (the glue draws them there); the vanilla ones (cherry petals,
// "Use Vanilla Textures") are the engine's own sprite sets.
use super::*;
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// `FallingLeafParticle.TAU`: one turn.
const TAU: f32 = std::f32::consts::TAU;
/// Ticks the fade-out lasts.
const FADE_DURATION: i32 = 16;
const WATER_FRICTION: f64 = 0.075;

/// Where the mod's textures sit in the mod-image slot: one row of 5 by 5 tiles,
/// a clear pixel between them, at this pixel row of the 1024 by 1024 slot (below
/// what Zoomify's settings screen draws there).
pub const SLOT_SIZE: f32 = 1024.0;
pub const TILE_ROW: u32 = 1008;
pub const TILE: u32 = 5;
pub const TILE_STEP: u32 = 7;
/// The tiles in the strip: `falling_leaf_1..5`, `falling_leaf_conifer_1..3`,
/// `falling_snow`.
pub const LEAF_TILES: [u32; 5] = [0, 1, 2, 3, 4];
pub const CONIFER_TILES: [u32; 3] = [5, 6, 7];
pub const SNOW_TILE: u32 = 8;
pub const TILES: u32 = 9;

/// The kind of particle (`Leaves.FALLING_LEAF`, `FALLING_CONIFER_LEAF`,
/// `FALLING_CHERRY`, `FALLING_SNOW`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Leaf,
    Conifer,
    Cherry,
    Snow,
}

// ---- the world's state the particles read ------------------------------------------------

/// `Wind.windX`, `Wind.windZ` (blocks a tick), as f32 bits.
static WIND_X: AtomicU32 = AtomicU32::new(0);
static WIND_Z: AtomicU32 = AtomicU32::new(0);
/// `level.isRaining()`.
static RAINING: AtomicBool = AtomicBool::new(false);

pub fn set_wind(x: f32, z: f32) {
    WIND_X.store(x.to_bits(), Ordering::Relaxed);
    WIND_Z.store(z.to_bits(), Ordering::Relaxed);
}
pub fn wind() -> (f32, f32) {
    (
        f32::from_bits(WIND_X.load(Ordering::Relaxed)),
        f32::from_bits(WIND_Z.load(Ordering::Relaxed)),
    )
}
pub fn set_raining(raining: bool) {
    RAINING.store(raining, Ordering::Relaxed);
}

/// `FluidState.getFlow`, which needs the game's fluid tables: the glue sets it.
type Flow = Box<dyn Fn(&World, [i32; 3]) -> [f64; 3] + Send + Sync>;
static FLOW: RwLock<Option<Flow>> = RwLock::new(None);

pub fn set_flow(flow: Flow) {
    if let Ok(mut f) = FLOW.write() {
        *f = Some(flow);
    }
}
fn flow(world: &World, b: [i32; 3]) -> [f64; 3] {
    FLOW.read()
        .ok()
        .and_then(|f| f.as_ref().map(|f| f(world, b)))
        .unwrap_or([0.0; 3])
}

/// The mod-image slot's place in the atlas: the engine's own sprite list leaves
/// the slot out, so the glue puts it in (again after each reload). Drawn
/// blended, as the leaves fade (`Layer.TRANSLUCENT`).
pub fn set_slot(engine: &mut ParticleEngine, rect: crate::hud::Rect) {
    engine.atlas.insert(crate::hud::MOD_IMAGE_SPRITE.into(), (rect, true));
}

// ---- making one -------------------------------------------------------------------------

/// What a new particle needs from the settings.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    /// `CONFIG.leafLifespan`.
    pub lifespan: i32,
    /// `CONFIG.getLeafSize()`.
    pub size: f32,
    /// `CONFIG.useVanillaTextures`.
    pub vanilla_textures: bool,
}

/// `FallingLeafParticle.BlockStateFactory.createParticle` and the constructor:
/// a particle of `kind` at `p`, coloured `rgb` (white for cherry petals and
/// snow), queued in the engine (`ParticleEngine.createParticle`: no distance
/// limit, as the original spawns them). False when its sprite is missing.
pub fn spawn(engine: &mut ParticleEngine, kind: Kind, p: [f64; 3], rgb: [f32; 3], look: Look) -> bool {
    // The sprite: the provider picks one of the set with the level's random.
    let (sprite, set, uv) = match kind {
        Kind::Cherry => match vanilla_sprite(engine, ParticleType::CherryLeaves) {
            Some((s, set)) => (s, Some(set), [0.0, 0.0, 1.0, 1.0]),
            None => return false,
        },
        Kind::Leaf if look.vanilla_textures => match vanilla_sprite(engine, ParticleType::TintedLeaves) {
            Some((s, set)) => (s, Some(set), [0.0, 0.0, 1.0, 1.0]),
            None => return false,
        },
        Kind::Leaf | Kind::Conifer | Kind::Snow => {
            let tiles: &[u32] = match kind {
                Kind::Conifer => &CONIFER_TILES,
                Kind::Snow => &[SNOW_TILE],
                _ => &LEAF_TILES,
            };
            let i = engine.random.next_int_bound(tiles.len() as i32).unwrap_or(0) as usize;
            (Sprite::Named(crate::hud::MOD_IMAGE_SPRITE.into()), None, tile_uv(tiles[i]))
        }
    };
    let mut random = LegacyRandom::new(engine.seeds.next_long());
    let mut particle = Particle::at(&mut random, ParticleType::Smoke, p);
    let r = &mut random;
    let (gravity, wind) = match kind {
        Kind::Cherry => (0.0175 + r.next_float() * 0.0050, 0.12 + r.next_float() * 0.8),
        Kind::Snow => (0.0125 + r.next_float() * 0.0125, 0.1 + r.next_float() * 0.1),
        _ => (0.08 + r.next_float() * 0.04, 0.6 + r.next_float() * 0.4),
    };
    // Accelerate over 3 to 7 seconds to at most 1 (cherry, snow) or 2.5 turns a second.
    let max_rotate_time = (3 + r.next_int_bound(4 + 1).unwrap_or(0)) * 20;
    let sign = if r.next_boolean() { -1.0 } else { 1.0 };
    let max_rotate_speed = if matches!(kind, Kind::Cherry | Kind::Snow) {
        sign * (0.1 + 0.4 * r.next_float()) * TAU / 20.0
    } else {
        sign * (0.1 + 2.4 * r.next_float()) * TAU / 20.0
    };
    particle.roll = r.next_float() * TAU;
    particle.o_roll = particle.roll;
    let mut size = if kind == Kind::Cherry { look.size / 2.0 } else { look.size };
    if r.next_boolean() {
        size *= 1.5;
    }
    particle.random = random;
    particle.velocity = [0.0; 3];
    particle.lifetime = look.lifespan;
    particle.gravity = gravity;
    particle.size = size;
    particle.size_curve = Size::Fixed;
    particle.rgba = [rgb[0], rgb[1], rgb[2], 1.0];
    particle.sprite = sprite;
    particle.set = set;
    particle.uv = uv;
    particle.behaviour = Behaviour::Controlled { translucent: true };
    particle.controller = Some(Box::new(Leaf {
        wind,
        max_rotate_speed,
        max_rotate_time,
        rotate_time: 0,
        in_water: false,
        stuck_in_ground: false,
    }));
    engine.push(particle);
    true
}

/// A random sprite of a vanilla set (`SpriteSet.get(random)`).
fn vanilla_sprite(engine: &mut ParticleEngine, ty: ParticleType) -> Option<(Sprite, u32)> {
    let set = engine.set_of(ty)?;
    let n = engine.set_sprites.get(set as usize)?.len();
    if n == 0 {
        return None;
    }
    let i = engine.random.next_int_bound(n as i32).unwrap_or(0) as usize;
    Some((Sprite::Index(engine.set_sprites[set as usize][i]), set))
}

/// The tile's corners in the slot, from 0 to 1.
fn tile_uv(tile: u32) -> [f32; 4] {
    let x = (tile * TILE_STEP) as f32;
    let y = TILE_ROW as f32;
    let t = TILE as f32;
    [x / SLOT_SIZE, y / SLOT_SIZE, (x + t) / SLOT_SIZE, (y + t) / SLOT_SIZE]
}

// ---- its tick ---------------------------------------------------------------------------

/// `FallingLeafParticle`'s own state.
#[derive(Clone, Debug)]
struct Leaf {
    /// `windCoefficient`: drag and lift.
    wind: f32,
    /// Turns a tick at full speed.
    max_rotate_speed: f32,
    max_rotate_time: i32,
    rotate_time: i32,
    in_water: bool,
    stuck_in_ground: bool,
}

impl ParticleController for Leaf {
    /// `FallingLeafParticle.tick`.
    fn tick(&mut self, p: &mut Particle, level: &ParticleLevel<'_>) -> ParticleUpdate {
        let mut update = ParticleUpdate::default();
        p.previous = p.position;
        p.o_roll = p.roll;
        p.age += 1;
        // The fade-out.
        if p.age >= p.lifetime + 1 - FADE_DURATION {
            p.rgba[3] -= 1.0 / FADE_DURATION as f32;
        }
        if p.age >= p.lifetime {
            p.remove();
            return update;
        }
        let b = ParticleLevel::block_at(p.position);
        let (fluid, height) = level.fluid(b);
        let y = p.position[1];
        if fluid == Fluid::Lava && f64::from(b[1]) + height >= y {
            update
                .spawned
                .push((ParticleOptions::simple(ParticleType::Lava), p.position, [0.0; 3]));
            p.remove();
            return update;
        }
        // Gravity, and the weight of rain where it falls on the leaf.
        let rained_on = RAINING.load(Ordering::Relaxed) && (level.world.light(b[0], b[1], b[2]) >> 4) >= 15;
        p.velocity[1] -= 0.04 * f64::from(p.gravity + if rained_on { 0.04 } else { 0.0 });
        let coefficient = f64::from(self.wind);
        if fluid == Fluid::Water && f64::from(b[1]) + height >= y - 0.1 {
            let water_y = f64::from(b[1]) + height;
            if !self.in_water {
                // Hit the water.
                self.in_water = true;
                // The original sets `y` alone, not the box: the next move puts it back.
                if (water_y - y).abs() < 0.2 {
                    p.position[1] = water_y;
                }
                p.velocity[1] *= 0.1;
                p.velocity[0] *= 0.5;
                p.velocity[2] *= 0.5;
                self.rotate_time = 0;
            } else {
                // Buoyancy: stay on top of the water.
                let depth = (water_y + 0.1 - p.position[1]).max(0.0);
                p.velocity[1] += depth * coefficient / 30.0;
            }
            let (_, source) = level.fluid_source(b);
            if !source {
                let push = flow(level.world, b);
                p.velocity[0] += (push[0] * 0.4 - p.velocity[0]) * coefficient / 60.0;
                p.velocity[2] += (push[2] * 0.4 - p.velocity[2]) * coefficient / 60.0;
            }
            for v in &mut p.velocity {
                *v *= 1.0 - WATER_FRICTION;
            }
        } else {
            self.in_water = false;
            if p.on_ground {
                self.rotate_time = 0;
            } else {
                // Spin in the air.
                self.rotate_time = (self.rotate_time + 1).min(self.max_rotate_time);
                p.roll += (self.rotate_time as f32 / self.max_rotate_time as f32) * self.max_rotate_speed;
            }
            // Toward the wind's speed, a little at a time.
            let (wx, wz) = wind();
            p.velocity[0] += (f64::from(wx) - p.velocity[0]) * coefficient / 60.0;
            p.velocity[2] += (f64::from(wz) - p.velocity[2]) * coefficient / 60.0;
        }
        let d = p.velocity;
        self.move_by(p, level, d);
        update
    }
}

impl Leaf {
    /// `FallingLeafParticle.move`: collide with blocks, stop sideways on a wall,
    /// and stick once it lies on the ground.
    fn move_by(&mut self, p: &mut Particle, level: &ParticleLevel<'_>, d: [f64; 3]) {
        if d == [0.0; 3] {
            return;
        }
        let moved = level.collide(p.bb, d);
        if d[0] != moved[0] {
            p.velocity[0] = 0.0;
        }
        if d[2] != moved[2] {
            p.velocity[2] = 0.0;
        }
        p.on_ground = d[1] != moved[1] && d[1] < 0.0;
        if !p.on_ground {
            self.stuck_in_ground = false;
        } else if !self.stuck_in_ground && moved[1].abs() < 1.0e-5 {
            self.stuck_in_ground = true;
        }
        if self.stuck_in_ground {
            p.velocity = [0.0; 3];
            return;
        }
        if moved != [0.0; 3] {
            p.bb = p.bb.move_by(Vec3::new(moved[0], moved[1], moved[2]));
            p.set_location_from_bb();
        }
    }
}

