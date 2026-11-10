// Emotecraft for Horizon: the emote on the player model, in the client library beside the
// model's poses (`crate::mobs::anim::emotecraft_pose`). What PlayerAnimationLibrary's mixins do:
// `PlayerModelMixin` (each body part through the emote's bone at the end of `setupAnim`),
// `PlayerCapeModelMixin` (the cape), `LivingEntityRendererMixin` (the `body` bone moves the whole
// model, before `AvatarRenderer.scale`) and `ItemInHandLayerMixin` (the item bones). The game
// fills the frames (entity id → the emote's pose this frame) once a frame; the render paths,
// which may run on other threads, read them here.
//
// This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0, with
// PlayerAnimationLibrary (Copyright (c) 2025 ZigyTheBird, MIT; see NOTICE).

use super::{Poses, State};
use crate::mobs::Mob;
use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
use std::cell::Cell;
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

thread_local! {
    /// Inside the game's own `base_transform_at` / `held_item_transform`, called again from
    /// its HEAD: let it run.
    static INSIDE_BASE: Cell<bool> = const { Cell::new(false) };
    static INSIDE_HELD: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f` (the hooked function itself) with its HEAD letting it through.
fn inside<R>(flag: &'static std::thread::LocalKey<Cell<bool>>, f: impl FnOnce() -> R) -> R {
    flag.with(|c| c.set(true));
    let r = f();
    flag.with(|c| c.set(false));
    r
}

/// A bone (`PlayerAnimBone`): position in pixels, rotation in radians, scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bone {
    pub pos: [f32; 3],
    pub rot: [f32; 3],
    pub scale: [f32; 3],
    pub bend: f32,
}

impl Default for Bone {
    fn default() -> Bone {
        Bone { pos: [0.0; 3], rot: [0.0; 3], scale: [1.0; 3], bend: 0.0 }
    }
}

/// A frame of an emote (the engine's `controller::Frame`, given by the game): `get3DTransform`.
pub trait Frame: Send + Sync {
    /// The bone named `name` with the emote over what `bone` holds.
    fn transform(&self, name: &str, bone: &mut Bone);
    /// The bone from its initial pose.
    fn bone(&self, name: &str) -> Bone {
        let mut b = Bone::default();
        self.transform(name, &mut b);
        b
    }
}

/// The id the window's own player model has (`Mob::new(0, …)` in `tick_player_model`).
pub const OWN_MODEL: u32 = 0;

static ANY: AtomicBool = AtomicBool::new(false);
static FRAMES: Mutex<Option<Arc<HashMap<u32, Arc<dyn Frame>>>>> = Mutex::new(None);

/// This frame's emotes, by the model's entity id (the own player is `OWN_MODEL`, previews have
/// ids of their own).
pub fn set_frames(frames: HashMap<u32, Arc<dyn Frame>>) {
    ANY.store(!frames.is_empty(), Ordering::Relaxed);
    *FRAMES.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(frames));
}

/// Adds or replaces one frame (a preview drawn now).
pub fn put_frame(id: u32, frame: Option<Arc<dyn Frame>>) {
    let mut g = FRAMES.lock().unwrap_or_else(|e| e.into_inner());
    let mut map: HashMap<u32, Arc<dyn Frame>> = g.as_deref().cloned().unwrap_or_default();
    match frame {
        Some(f) => {
            map.insert(id, f);
        }
        None => {
            map.remove(&id);
        }
    }
    ANY.store(!map.is_empty(), Ordering::Relaxed);
    *g = Some(Arc::new(map));
}

fn avatar(m: &Mob) -> bool {
    matches!(&*m.kind.id, "minecraft:player" | "minecraft:mannequin")
}

/// The emote's pose for a model, if its mob is an avatar playing one.
pub fn frame_of(m: &Mob) -> Option<Arc<dyn Frame>> {
    if !ANY.load(Ordering::Relaxed) || !avatar(m) {
        return None;
    }
    let map = FRAMES.lock().unwrap_or_else(|e| e.into_inner()).clone()?;
    map.get(&m.id).cloned()
}

/// A part's pose when the model is made (`PartPose initialPose`). The body's layer key `main` is
/// the type's own model; the other keys (`player_slim#main`, `player#cape`) are its layers.
fn initial(m: &Mob, layer: &str, part: &str) -> [f32; 9] {
    let found = if layer == "main" { find(&m.kind.model, part) } else { m.kind.layer(layer).and_then(|model| find(model, part)) };
    found.map_or([0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0], |p| p.pose)
}

fn find<'a>(p: &'a crate::mobs::Part, name: &str) -> Option<&'a crate::mobs::Part> {
    if p.name == name {
        return Some(p);
    }
    p.children.iter().find_map(|c| find(c, name))
}

/// `RenderUtil.copyVanillaPart`.
fn to_bone(p: &[f32; 9], init: &[f32; 9]) -> Bone {
    Bone { pos: [p[0] - init[0], -(p[1] - init[1]), p[2] - init[2]], rot: [p[3], p[4], p[5]], scale: [p[6], p[7], p[8]], bend: 0.0 }
}

/// `RenderUtil.translatePartToBone`.
fn to_part(b: &Bone, init: &[f32; 9]) -> [f32; 9] {
    [b.pos[0] + init[0], -b.pos[1] + init[1], b.pos[2] + init[2], b.rot[0], b.rot[1] + init[4], b.rot[2], b.scale[0], b.scale[1], b.scale[2]]
}

/// RETURN of `anim::setup`: each body part the model has through its bone, and the cape.
pub fn apply(m: &Mob, s: &State, p: &mut Poses) {
    let Some(frame) = frame_of(m) else { return };
    let main = if m.slim { "player_slim#main" } else { "main" };
    for (part, bone) in [("head", "head"), ("body", "torso"), ("right_arm", "right_arm"), ("left_arm", "left_arm"), ("right_leg", "right_leg"), ("left_leg", "left_leg")] {
        if !p.has(part) {
            continue;
        }
        let init = initial(m, main, part);
        let mut b = to_bone(p.get(part), &init);
        frame.transform(bone, &mut b);
        *p.get(part) = to_part(&b, &init);
    }
    if p.has("cape") {
        cape(m, s, p, &*frame);
    }
}

/// `PlayerCapeModelMixin`, at the end of `PlayerCapeModel.setupAnim`, after the cape's flap. The
/// game turns the cape by its flap after this runs (`render::cape`): the pose is set so that
/// what the flap then gives is the emote's.
fn cape(m: &Mob, s: &State, p: &mut Poses, frame: &dyn Frame) {
    let alpha = s.partial;
    let a = f64::from(alpha);
    let at = [
        m.previous.x + (m.position.x - m.previous.x) * a,
        m.previous.y + (m.position.y - m.previous.y) * a,
        m.previous.z + (m.position.z - m.previous.z) * a,
    ];
    let body = m.previous_body_yaw + crate::mobs::living::wrap(m.body_yaw - m.previous_body_yaw) * alpha;
    let ticks = m.extra.fall_fly_ticks as f32 + alpha;
    let gliding = (ticks * ticks / 100.0).clamp(0.0, 1.0);
    let [flap, lean, lean2] = m.extra.avatar.cape(alpha, at, body, gliding);
    let by = glam::Quat::from_rotation_y(-PI)
        * glam::Quat::from_rotation_x((6.0 + lean / 2.0 + flap).to_radians())
        * glam::Quat::from_rotation_z((lean2 / 2.0).to_radians())
        * glam::Quat::from_rotation_y((180.0 - lean2 / 2.0).to_radians());
    let pose = *p.get("cape");
    let now = glam::Quat::from_euler(glam::EulerRot::ZYX, pose[5], pose[4], pose[3]);
    let (z, y, x) = (now * by).to_euler(glam::EulerRot::ZYX);
    let init = initial(m, "player#cape", "cape");
    let mut b = Bone { pos: [pose[0] - init[0], -(pose[1] - init[1]), pose[2] - init[2]], rot: [x, y, z], scale: [pose[6], pose[7], pose[8]], bend: 0.0 };
    b.rot[0] -= PI;
    b.rot[2] -= PI;
    b.rot[0] *= -1.0;
    b.rot[1] *= -1.0;
    frame.transform("cape", &mut b);
    b.rot[0] *= -1.0;
    b.rot[1] *= -1.0;
    b.rot[0] += PI;
    b.rot[2] += PI;
    // `RenderUtil.translatePartToCape`, then undo the flap the game adds next.
    let target = glam::Quat::from_euler(glam::EulerRot::ZYX, b.rot[2], b.rot[1], b.rot[0]);
    let (z, y, x) = (target * by.inverse()).to_euler(glam::EulerRot::ZYX);
    *p.get("cape") = [b.pos[0] + init[0], -(b.pos[1] + init[1]), b.pos[2] + init[2], x, y, z, b.scale[0], b.scale[1], b.scale[2]];
}

/// HEAD of `base_transform_at`: for an emoting avatar, the game's transform with the `body`
/// bone put in.
pub fn base_head(m: &Mob, alpha: f32, origin: glam::Mat4) -> ControlFlow<glam::Mat4> {
    if INSIDE_BASE.with(Cell::get) {
        return ControlFlow::Continue(());
    }
    let Some(frame) = frame_of(m) else { return ControlFlow::Continue(()) };
    let value = inside(&INSIDE_BASE, || crate::mobs::render::base_transform_at(m, alpha, origin));
    ControlFlow::Break(base(value, &*frame))
}

/// `LivingEntityRendererMixin`: the `body` bone, between the model's flip and
/// `AvatarRenderer.scale` (`value` ends with `flip · scale(0.9375) · translate(0, -1.501, 0)`).
fn base(value: glam::Mat4, frame: &dyn Frame) -> glam::Mat4 {
    let b = frame.bone("body");
    let flip = glam::Mat4::from_scale(glam::Vec3::new(-1.0, -1.0, 1.0));
    let tail = glam::Mat4::from_scale(glam::Vec3::splat(0.9375)) * glam::Mat4::from_translation(glam::Vec3::new(0.0, -1.501, 0.0));
    let bone = flip
        * glam::Mat4::from_translation(glam::Vec3::new(-b.pos[0] / 16.0, b.pos[1] / 16.0 + 0.75, b.pos[2] / 16.0))
        * glam::Mat4::from_rotation_z(b.rot[2])
        * glam::Mat4::from_rotation_y(-b.rot[1])
        * glam::Mat4::from_rotation_x(-b.rot[0])
        * glam::Mat4::from_scale(glam::Vec3::new(b.scale[0], b.scale[1], b.scale[2]))
        * glam::Mat4::from_translation(glam::Vec3::new(0.0, -0.75, 0.0))
        * flip;
    value * tail.inverse() * bone * tail
}

/// HEAD of `held_item_transform`: for an emoting avatar, the game's transform with the item bone.
pub fn held_head(m: &Mob, alpha: f32, camera: glam::DVec3, data: &crate::mobs::MobData, left: bool) -> ControlFlow<Option<glam::Mat4>> {
    if INSIDE_HELD.with(Cell::get) {
        return ControlFlow::Continue(());
    }
    let Some(frame) = frame_of(m) else { return ControlFlow::Continue(()) };
    let value = inside(&INSIDE_HELD, || crate::mobs::render::held_item_transform(m, alpha, camera, data, left));
    ControlFlow::Break(value.map(|t| held_item(t, &*frame, left)))
}

/// `ItemInHandLayerMixin`: the item bone moves the held item before the hand's turn
/// (`rotateDegrees(XP, -90)`), and turns and scales it before it is drawn.
fn held_item(t: glam::Mat4, frame: &dyn Frame, left: bool) -> glam::Mat4 {
    let b = frame.bone(if left { "left_item" } else { "right_item" });
    let side = if left { -1.0 } else { 1.0 };
    // `hand_base`'s turn and offset after `translateToHand`.
    let k = glam::Mat4::from_rotation_x((-90.0f32).to_radians())
        * glam::Mat4::from_rotation_y(PI)
        * glam::Mat4::from_translation(glam::Vec3::new(side / 16.0, 2.0 / 16.0, -10.0 / 16.0));
    let moved = glam::Mat4::from_translation(glam::Vec3::new(b.pos[0] / 16.0, -b.pos[1] / 16.0, b.pos[2] / 16.0));
    let turned = glam::Mat4::from_rotation_z(-b.rot[1]) * glam::Mat4::from_rotation_y(-b.rot[2]) * glam::Mat4::from_rotation_x(-b.rot[0]);
    let scaled = glam::Mat4::from_scale(glam::Vec3::new(b.scale[0], b.scale[1], b.scale[2]));
    t * k.inverse() * moved * k * turned * scaled
}
