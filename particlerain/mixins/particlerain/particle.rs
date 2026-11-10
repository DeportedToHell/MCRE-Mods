// Particle Rain inside Horizon's particle engine: the frame's weather particles drawn with the
// game's particles (their quads are built by the game side, which knows the world, and handed
// over here for the engine's `render`), and what the original does to the game's own particles:
// the wind on the particles it spawns from the game's types (`WindManager.track`), and the
// water's colour and the grey splash sprites on rain drops, splashes and water drips
// (`WaterDropParticleMixin`, `DripParticleMixin`). Spliced into horizon-client-render's
// particles.rs as `particlerain_particle`, so it can reach what the engine keeps private.
//
// Particle Rain by PigCart (https://github.com/PigCart/particle-rain), MIT.

use super::*;
use std::cell::RefCell;

thread_local! {
    /// The weather particles' quads for the frame being drawn.
    static FRAME: RefCell<Vec<Vertex>> = const { RefCell::new(Vec::new()) };
}

/// The game side's quads for this frame (camera-relative, as the engine's own).
pub fn set_frame(vertices: Vec<Vertex>) {
    FRAME.with(|f| *f.borrow_mut() = vertices);
}

/// Into the engine's output, once (the particle pass of the frame).
pub fn append(out: &mut Vec<Vertex>) {
    FRAME.with(|f| out.append(&mut f.borrow_mut()));
}

// ---- the game's own particles ---------------------------------------------------------------

/// The mark of a particle Particle Rain spawned from a game type, in its sub-rectangle's left edge
/// (a negative zero-width offset nothing draws): the index of its settings, plus one.
fn tag_of(p: &Particle) -> Option<u32> {
    let bits = p.uv[0].to_bits();
    (bits & 0x8000_0000 != 0 && bits & 0x7FFF_0000 == 0 && bits & 0xFFFF != 0).then(|| (bits & 0xFFFF) - 1)
}

/// `ParticleEngine.createParticle` of a game type, then `WindManager.track`: the spawn's wind
/// added to its motion, and the particle marked with `tag` for the wind of later ticks. False
/// when nothing was made.
pub fn create_tracked(engine: &mut ParticleEngine, options: &ParticleOptions, p: [f64; 3], tag: u32, wind: [f64; 2]) -> bool {
    let before = engine.pending.len();
    engine.create(options, p, [0.0; 3]);
    if engine.pending.len() <= before {
        return false;
    }
    if let Some(q) = engine.pending.last_mut() {
        q.velocity[0] += wind[0];
        q.velocity[2] += wind[1];
        q.uv[0] = f32::from_bits(0x8000_0000 | (tag.min(0xFFFE) + 1));
    }
    true
}

/// `WindManager.tick`: each marked particle still alive and younger than `MAX_TICKS`, with its tag.
pub fn tracked(engine: &mut ParticleEngine, mut f: impl FnMut(&mut Particle, u32)) {
    for p in engine.particles.iter_mut().chain(engine.pending.iter_mut()) {
        if let Some(tag) = tag_of(p)
            && !p.removed
            && p.age <= 6000
        {
            f(p, tag);
        }
    }
}

/// A particle's position (`ParticleAccessor`).
pub fn position(p: &Particle) -> [f64; 3] {
    p.position
}

/// The water's colour on the game's water particles just made (this tick's, not yet ticked):
/// rain drops and splashes take the grey `particlerain:splash_N` sprite (`splash` picks N) and
/// the colour, water drips the colour.
pub fn fresh_water(engine: &mut ParticleEngine, mut tint: impl FnMut([f64; 3]) -> Option<[f32; 3]>, mut splash: impl FnMut() -> Option<String>) {
    use ParticleType as T;
    for p in engine.particles.iter_mut() {
        if p.age != 0 || p.removed {
            continue;
        }
        let drop = matches!(p.ty, T::Rain | T::Splash);
        if !drop && !matches!(p.ty, T::DrippingWater | T::FallingWater | T::DrippingDripstoneWater | T::FallingDripstoneWater) {
            continue;
        }
        if drop && let Some(name) = splash() {
            p.sprite = Sprite::Named(name);
            p.set = None;
        }
        if let Some(rgb) = tint(p.position) {
            p.rgba[0] = rgb[0];
            p.rgba[1] = rgb[1];
            p.rgba[2] = rgb[2];
        }
    }
}

