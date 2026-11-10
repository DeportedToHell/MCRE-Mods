//! Playing an emote on an avatar: PlayerAnimationLibrary's `AnimationController`,
//! `HumanoidAnimationController` (the player's bones, their pivots, the torso bend carried to
//! the upper body) and the bones (`PlayerAnimBone`, `ToggleablePlayerAnimBone`,
//! `AdvancedPlayerAnimBone`: per axis, only what the emote keys replaces the vanilla pose, and
//! `beginTick`/`endTick` blend from and back to it), driven as Emotecraft's `EmotePlayer` drives
//! it. `Frame` is what a frame of it gives the renderer.
//!
//! Ported from PlayerAnimationLibrary (Copyright (c) 2025 ZigyTheBird, MIT; files based on
//! GeckoLib, Copyright (c) 2024 GeckoLib, MIT, and playerAnimator, Copyright (c) 2022 KosmX,
//! MIT), see NOTICE.

use crate::anim::{Animation, Format, Keyframe, LoopType, last_time};
use crate::easing::Easing;
use crate::molang::{self, Runtime};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

// ---- JOML's Matrix4f, as far as the bones use it ------------------------------------------

/// A column-major 4x4 float matrix (`m[column][row]`), multiplied as JOML does (`this * other`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4(pub [[f32; 4]; 4]);

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);
    pub fn mul(&self, o: &Mat4) -> Mat4 {
        let mut r = [[0f32; 4]; 4];
        for (c, col) in r.iter_mut().enumerate() {
            for (row, v) in col.iter_mut().enumerate() {
                *v = (0..4).map(|k| self.0[k][row] * o.0[c][k]).sum();
            }
        }
        Mat4(r)
    }
    pub fn translation(x: f32, y: f32, z: f32) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.0[3] = [x, y, z, 1.0];
        m
    }
    pub fn scaling(x: f32, y: f32, z: f32) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.0[0][0] = x;
        m.0[1][1] = y;
        m.0[2][2] = z;
        m
    }
    pub fn rot_x(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        Mat4([[1.0, 0.0, 0.0, 0.0], [0.0, c, s, 0.0], [0.0, -s, c, 0.0], [0.0, 0.0, 0.0, 1.0]])
    }
    pub fn rot_y(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        Mat4([[c, 0.0, -s, 0.0], [0.0, 1.0, 0.0, 0.0], [s, 0.0, c, 0.0], [0.0, 0.0, 0.0, 1.0]])
    }
    pub fn rot_z(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        Mat4([[c, s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]])
    }
    pub fn translate(&mut self, x: f32, y: f32, z: f32) {
        *self = self.mul(&Mat4::translation(x, y, z));
    }
    pub fn scale(&mut self, x: f32, y: f32, z: f32) {
        *self = self.mul(&Mat4::scaling(x, y, z));
    }
    /// `rotateZYX(z, y, x)`: `this * Rz * Ry * Rx`.
    pub fn rotate_zyx(&mut self, z: f32, y: f32, x: f32) {
        *self = self.mul(&Mat4::rot_z(z)).mul(&Mat4::rot_y(y)).mul(&Mat4::rot_x(x));
    }
    /// `getEulerAnglesZYX`.
    pub fn euler_zyx(&self) -> [f32; 3] {
        let m = &self.0;
        [m[1][2].atan2(m[2][2]), (-m[0][2]).atan2((1.0 - m[0][2] * m[0][2]).max(0.0).sqrt()), m[0][1].atan2(m[0][0])]
    }
    /// `getScale`.
    pub fn get_scale(&self) -> [f32; 3] {
        let m = &self.0;
        let len = |c: &[f32; 4]| (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
        [len(&m[0]), len(&m[1]), len(&m[2])]
    }
}

// ---- bones ----------------------------------------------------------------------------------

/// `PlayerAnimBone`: position (pixels), rotation (radians), scale, and the deprecated bend.
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

impl Bone {
    /// `applyOtherBone`: positions and rotations add, scales multiply.
    pub fn apply_other(&mut self, o: &Bone) {
        for i in 0..3 {
            self.pos[i] += o.pos[i];
            self.rot[i] += o.rot[i];
            self.scale[i] *= o.scale[i];
        }
        self.bend += o.bend;
    }
}

/// `AdvancedPlayerAnimBone`: which axes the emote drives, and the begin/end blend lengths.
#[derive(Clone, Copy, Debug)]
struct AdvBone {
    b: Bone,
    /// pos x y z, rot x y z, scale x y z, bend.
    enabled: [bool; 10],
    transition: [Option<f32>; 10],
}

impl Default for AdvBone {
    fn default() -> AdvBone {
        AdvBone { b: Bone::default(), enabled: [true; 10], transition: [None; 10] }
    }
}

/// `HumanoidAnimationController.BONE_POSITIONS`.
pub fn bone_position(name: &str) -> Option<[f32; 3]> {
    Some(match name {
        "right_item" => [6.0, 12.0, -2.0],
        "left_item" => [-6.0, 12.0, -2.0],
        "right_arm" => [5.0, 22.0, 0.0],
        "left_arm" => [-5.0, 22.0, 0.0],
        "left_leg" => [-2.0, 12.0, 0.0],
        "right_leg" => [2.0, 12.0, 0.0],
        "torso" | "head" => [0.0, 24.0, 0.0],
        "body" => [0.0, 12.0, 0.0],
        "cape" | "elytra" => [0.0, 24.0, 2.0],
        _ => return None,
    })
}

/// The bones `registerBones` registers, and the top ones the torso bend carries.
pub const BONES: [&str; 11] = ["body", "right_arm", "left_arm", "right_leg", "left_leg", "head", "torso", "right_item", "left_item", "cape", "elytra"];
const TOP_BONES: [&str; 4] = ["right_arm", "left_arm", "head", "cape"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Stopped,
    Running,
    Paused,
}

#[derive(Clone, Copy, PartialEq)]
enum Ty {
    Rotation,
    Position,
    Scale,
    Bend,
}

/// What a frame of the emote poses: the bones' values and what their axes say, at a time.
#[derive(Clone, Debug)]
pub struct Frame {
    anim: Arc<Animation>,
    bones: BTreeMap<&'static str, AdvBone>,
    pivots: BTreeMap<String, ([f32; 3], Bone)>,
    active: BTreeSet<String>,
    ticks: f32,
    torso_bend: f32,
    torso_bend_sign: i32,
}

impl Frame {
    fn bone_position(&self, name: &str) -> [f32; 3] {
        bone_position(name).or_else(|| self.pivots.get(name).map(|p| p.0)).unwrap_or([0.0; 3])
    }

    /// `get3DTransform(bone)` of the emote's layer: `bone` holds the vanilla pose (or the
    /// initial one); the axes the emote drives take its values, blended at the begin and end.
    pub fn transform(&self, name: &str, bone: &mut Bone) {
        if self.active.contains(name) {
            if let Some(adv) = BONES.iter().find(|b| **b == name).and_then(|b| self.bones.get(b)) {
                let a = &self.anim;
                if let Some(begin) = a.begin_tick.filter(|b| *b > self.ticks) {
                    let _ = begin;
                    begin_or_end_lerp(bone, adv, self.ticks, None, name);
                } else if let Some(end) = a.end_tick.filter(|e| !a.loop_type.plays_again() && *e <= self.ticks) {
                    begin_or_end_lerp(bone, adv, self.ticks - end, Some(a), name);
                } else {
                    copy_if_enabled(bone, adv);
                }
            } else if let Some((_, p)) = self.pivots.get(name) {
                *bone = *p;
            }
        }
        if self.torso_bend_sign != 0 && TOP_BONES.contains(&name) {
            let mut m = Mat4::translation(0.0, 18.0, 0.0);
            m = m.mul(&Mat4::rot_x(self.torso_bend));
            m.translate(0.0, -18.0, 0.0);
            apply_matrix(bone, m, self.bone_position(name));
        }
    }

    /// The bone with nothing under it (`setToInitialPose`, then `get3DTransform`).
    pub fn bone(&self, name: &str) -> Bone {
        let mut b = Bone::default();
        self.transform(name, &mut b);
        b
    }

    pub fn animation(&self) -> &Arc<Animation> {
        &self.anim
    }
}

fn copy_if_enabled(bone: &mut Bone, adv: &AdvBone) {
    for i in 0..3 {
        if adv.enabled[i] {
            bone.pos[i] = adv.b.pos[i];
        }
        if adv.enabled[3 + i] {
            bone.rot[i] = adv.b.rot[i];
        }
        if adv.enabled[6 + i] {
            bone.scale[i] = adv.b.scale[i];
        }
    }
    if adv.enabled[9] {
        bone.bend = adv.b.bend;
    }
}

/// `PlayerAnimBone.beginOrEndTickLerp`.
fn begin_or_end_lerp(bone: &mut Bone, adv: &AdvBone, time: f32, anim: Option<&Arc<Animation>>, name: &str) {
    let lerp = |start: f32, end: f32, transition: Option<f32>, ty: Ty, axis: usize| -> f32 {
        let mut easing = Easing::InOutSine;
        let (mut start, mut end, mut transition) = (start, end, transition);
        if let Some(a) = anim {
            std::mem::swap(&mut start, &mut end);
            if transition.is_none() {
                transition = Some(a.length - a.end_tick.unwrap_or(0.0));
            }
            if a.ease_before == Some(false) {
                if let Some(b) = a.bones.get(name) {
                    let list = match ty {
                        Ty::Bend => &b.bend,
                        Ty::Rotation => b.rotation.axis(axis),
                        Ty::Scale => b.scale.axis(axis),
                        Ty::Position => b.position.axis(axis),
                    };
                    if let Some(k) = list.last() {
                        easing = k.easing;
                    }
                }
            }
            if matches!(easing, Easing::Bezier | Easing::CatmullRom) {
                easing = Easing::InOutSine;
            }
        }
        match transition {
            None => end,
            Some(t) => easing.apply_simple(start, end, time / t),
        }
    };
    for i in 0..3 {
        if adv.enabled[i] {
            bone.pos[i] = lerp(bone.pos[i], adv.b.pos[i], adv.transition[i], Ty::Position, i);
        }
        if adv.enabled[3 + i] {
            bone.rot[i] = lerp(bone.rot[i], adv.b.rot[i], adv.transition[3 + i], Ty::Rotation, i);
        }
        if adv.enabled[6 + i] {
            bone.scale[i] = lerp(bone.scale[i], adv.b.scale[i], adv.transition[6 + i], Ty::Scale, i);
        }
    }
    if adv.enabled[9] {
        bone.bend = lerp(bone.bend, adv.b.bend, adv.transition[9], Ty::Bend, 1);
    }
}

/// `MatrixUtil.applyMatrixToBone`.
fn apply_matrix(bone: &mut Bone, mut m: Mat4, pivot: [f32; 3]) {
    m.translate(pivot[0], pivot[1], -pivot[2]);
    m.translate(-bone.pos[0], bone.pos[1], -bone.pos[2]);
    if bone.rot != [0.0; 3] {
        m.rotate_zyx(bone.rot[2], bone.rot[1], bone.rot[0]);
    }
    let t = m.0[3];
    bone.pos = [-t[0] + pivot[0], t[1] - pivot[1], -t[2] + pivot[2]];
    bone.rot = m.euler_zyx();
    let s = m.get_scale();
    for i in 0..3 {
        bone.scale[i] *= s[i];
    }
}

/// `MatrixUtil.prepMatrixForBone`.
fn prep_matrix(m: &mut Mat4, bone: &Bone, pivot: [f32; 3]) {
    m.translate(pivot[0], pivot[1], -pivot[2]);
    m.translate(-bone.pos[0], bone.pos[1], -bone.pos[2]);
    if bone.rot != [0.0; 3] {
        m.rotate_zyx(bone.rot[2], bone.rot[1], bone.rot[0]);
    }
    m.scale(bone.scale[0], bone.scale[1], bone.scale[2]);
    m.translate(-pivot[0], -pivot[1], pivot[2]);
}

/// Where a keyframe list is at a time (`KeyframeLocation`): the keyframe (or the loop seam's),
/// the time into it, and whether it is the list's first or last.
struct Location {
    frame: Keyframe,
    at: f32,
    first: bool,
    last: bool,
}

/// `EmotePlayer`: an avatar's emote layer.
pub struct Controller {
    state: State,
    current: Option<Arc<Animation>>,
    /// The loop type the emote was started with (`LoopType.DEFAULT` is the animation's own).
    loop_type: Option<LoopType>,
    triggered: Option<(Arc<Animation>, Option<LoopType>)>,
    needs_reload: bool,
    tick: i32,
    start_from: f32,
    partial: f32,
    loop_started: bool,
    bones: BTreeMap<&'static str, AdvBone>,
    pivots: BTreeMap<String, ([f32; 3], Bone)>,
    active: BTreeSet<String>,
    torso_bend: f32,
    torso_bend_sign: i32,
    rt: Runtime,
}

impl Default for Controller {
    fn default() -> Controller {
        Controller::new()
    }
}

impl Controller {
    pub fn new() -> Controller {
        Controller {
            state: State::Stopped,
            current: None,
            loop_type: None,
            triggered: None,
            needs_reload: false,
            tick: 0,
            start_from: 0.0,
            partial: 0.0,
            loop_started: false,
            bones: BONES.iter().map(|b| (*b, AdvBone::default())).collect(),
            pivots: BTreeMap::new(),
            active: BTreeSet::new(),
            torso_bend: 0.0,
            torso_bend_sign: 0,
            rt: Runtime::new(),
        }
    }

    /// `isActive`: running or paused.
    pub fn is_active(&self) -> bool {
        self.state != State::Stopped
    }

    /// The emote playing (`getCurrentAnimationInstance`).
    pub fn current(&self) -> Option<&Arc<Animation>> {
        self.current.as_ref().or(self.triggered.as_ref().map(|t| &t.0))
    }

    /// `getAnimationTicks`.
    pub fn ticks(&self) -> f32 {
        self.tick as f32 + self.start_from + self.partial
    }

    fn effective_loop(&self, a: &Animation) -> LoopType {
        self.loop_type.unwrap_or(a.loop_type)
    }

    /// `triggerAnimation(RawAnimation.begin().then(emote, loopType), tick)`; `loop_type` none is
    /// `LoopType.DEFAULT`.
    pub fn play(&mut self, anim: Arc<Animation>, loop_type: Option<LoopType>, tick: f32) {
        self.stop();
        self.triggered = Some((anim, loop_type));
        self.needs_reload = true;
        self.state = State::Running;
        self.tick = 0;
        self.start_from = tick;
    }

    /// `EmotePlayer.stop`.
    pub fn stop(&mut self) {
        self.state = State::Stopped;
        if self.triggered.is_some() {
            self.current = None;
            self.triggered = None;
            self.needs_reload = true;
        }
    }

    pub fn pause(&mut self) {
        self.state = State::Paused;
    }
    pub fn unpause(&mut self) {
        if self.state == State::Paused {
            self.state = State::Running;
        }
    }
    pub fn is_paused(&self) -> bool {
        self.state == State::Paused
    }

    /// `handleAnimation`: false is `PlayState.STOP`.
    fn handle(&mut self) -> bool {
        if let Some((anim, loop_type)) = self.triggered.clone() {
            // `setAnimation`, keeping a paused state (`EmotePlayer.setAnimation`).
            if self.needs_reload {
                let saved = self.state;
                self.current = Some(anim);
                self.loop_type = loop_type;
                self.tick = 0;
                self.state = State::Running;
                self.setup_new();
                self.needs_reload = false;
                self.state = saved;
            }
            let finished = self.state == State::Stopped;
            if !finished {
                return true;
            }
            self.triggered = None;
            self.needs_reload = true;
        }
        false
    }

    /// `tick`: once a game tick.
    pub fn tick(&mut self) {
        self.handle();
        if self.state == State::Running {
            self.tick += 1;
        }
    }

    /// `process`: the bones at `partial` into the tick. False once the emote stopped.
    pub fn process(&mut self, partial: f32) -> bool {
        self.partial = partial;
        let adjusted = (partial + self.start_from + self.tick as f32).max(0.0);
        if !self.handle() || self.current.is_none() {
            self.state = State::Stopped;
            return false;
        }
        if self.state == State::Running {
            self.process_current(adjusted);
        }
        // `HumanoidAnimationController.process`.
        self.torso_bend = self.bones.get("torso").map_or(0.0, |b| b.b.bend);
        let apply = self.current.as_ref().is_some_and(|a| a.apply_bend);
        self.torso_bend_sign = if self.torso_bend.abs() > 0.001 && apply { self.torso_bend.signum() as i32 } else { 0 };
        self.is_active()
    }

    /// `setupNewAnimation`.
    fn setup_new(&mut self) {
        self.loop_started = false;
        let Some(a) = self.current.clone() else { return };
        self.active.clear();
        for (name, bone) in self.bones.iter_mut() {
            let on = a.bones.contains_key(*name);
            bone.enabled = [on; 10];
        }
        for (name, ba) in &a.bones {
            if let Some(bone) = self.bones.get_mut(name.as_str()) {
                self.active.insert(name.clone());
                if a.disable_axis {
                    bone.enabled = [
                        !ba.position.x.is_empty(),
                        !ba.position.y.is_empty(),
                        !ba.position.z.is_empty(),
                        !ba.rotation.x.is_empty(),
                        !ba.rotation.y.is_empty(),
                        !ba.rotation.z.is_empty(),
                        !ba.scale.x.is_empty(),
                        !ba.scale.y.is_empty(),
                        !ba.scale.z.is_empty(),
                        !ba.bend.is_empty(),
                    ];
                } else {
                    bone.enabled = [true; 10];
                }
            } else if a.pivots.contains_key(name) {
                self.active.insert(name.clone());
            }
        }
        for name in a.parents.keys() {
            if let Some(b) = self.bones.get_mut(name.as_str()) {
                b.enabled = [true; 10];
            }
        }
        self.pivots = a.pivots.iter().map(|(k, v)| (k.clone(), (*v, Bone::default()))).collect();
    }

    fn reset_bones(&mut self) {
        for b in self.bones.values_mut() {
            b.b = Bone::default();
        }
        for p in self.pivots.values_mut() {
            p.1 = Bone::default();
        }
    }

    /// `processCurrentAnimation`.
    fn process_current(&mut self, mut adjusted: f32) {
        let Some(a) = self.current.clone() else {
            self.state = State::Stopped;
            return;
        };
        let lt = self.effective_loop(&a);
        if adjusted >= a.length {
            if lt.plays_again() {
                if lt == LoopType::HoldOnLastFrame {
                    // `HOLD_ON_LAST_FRAME` pauses the controller.
                    self.state = State::Paused;
                }
                if self.state != State::Paused {
                    self.tick = 0;
                    self.start_from = lt.restart_tick();
                    adjusted = self.start_from;
                    self.start_from -= self.partial;
                    self.loop_started = true;
                }
            } else {
                self.state = State::Stopped;
                self.current = None;
                self.reset_bones();
                return;
            }
        }
        self.reset_bones();
        self.rt.anim_time = f64::from(self.ticks() / 20.0);
        let pa_loop = a.format == Format::PlayerAnimator && lt.plays_again();
        for (name, ba) in &a.bones {
            let advanced = self.bones.contains_key(name.as_str());
            if !advanced && !self.pivots.contains_key(name) {
                continue;
            }
            let mut out = Bone::default();
            let mut trans = [None; 10];
            let stacks = [(&ba.rotation, Ty::Rotation, 3), (&ba.position, Ty::Position, 0), (&ba.scale, Ty::Scale, 6)];
            for (stack, ty, base) in stacks {
                for axis in 0..3 {
                    let (v, t) = self.value(&a, lt, pa_loop, stack.axis(axis), adjusted, ty);
                    match ty {
                        Ty::Rotation => out.rot[axis] = v,
                        Ty::Position => out.pos[axis] = v,
                        _ => out.scale[axis] = v,
                    }
                    trans[base + axis] = t;
                }
            }
            let (bend, t) = self.value(&a, lt, pa_loop, &ba.bend, adjusted, Ty::Bend);
            out.bend = bend;
            trans[9] = t;
            if let Some(b) = self.bones.get_mut(name.as_str()) {
                b.b = out;
                b.transition = trans;
            } else if let Some(p) = self.pivots.get_mut(name) {
                p.1 = out;
            }
        }
        self.apply_pivots(&a);
    }

    /// `applyCustomPivotPoints` / `processBoneHierarchy`.
    fn apply_pivots(&mut self, a: &Animation) {
        if a.parents.is_empty() {
            return;
        }
        let names: Vec<String> = self.bones.keys().map(|s| s.to_string()).chain(self.pivots.keys().cloned()).collect();
        let mut done = BTreeSet::new();
        for n in names {
            self.hierarchy(&n, a, &mut done, 0);
        }
    }

    fn get_bone(&self, name: &str) -> Option<Bone> {
        self.pivots.get(name).map(|p| p.1).or_else(|| self.bones.get(name).map(|b| b.b))
    }

    fn position_of(&self, name: &str) -> [f32; 3] {
        bone_position(name).or_else(|| self.pivots.get(name).map(|p| p.0)).unwrap_or([0.0; 3])
    }

    fn hierarchy(&mut self, name: &str, a: &Animation, done: &mut BTreeSet<String>, depth: u32) {
        if done.contains(name) || depth > 64 {
            return;
        }
        let Some(parent) = a.parents.get(name).cloned() else {
            done.insert(name.to_owned());
            return;
        };
        if self.get_bone(&parent).is_none() {
            log::error!("[Emotecraft] Parent {parent} not found for {name}");
            return;
        }
        self.hierarchy(&parent, a, done, depth + 1);
        let Some(pb) = self.get_bone(&parent) else { return };
        self.active.insert(name.to_owned());
        let pivot = if self.pivots.contains_key(&parent) { self.pivots[&parent].0 } else { self.position_of(&parent) };
        let mut m = Mat4::IDENTITY;
        prep_matrix(&mut m, &pb, pivot);
        let child_pos = self.position_of(name);
        if let Some(b) = self.bones.get_mut(name) {
            apply_matrix(&mut b.b, m, child_pos);
        } else if let Some(p) = self.pivots.get_mut(name) {
            apply_matrix(&mut p.1, m, child_pos);
        }
        done.insert(name.to_owned());
    }

    /// `computeAnimValue`: an axis' value, and the begin/end blend length it sets.
    fn value(&mut self, a: &Arc<Animation>, lt: LoopType, pa_loop: bool, frames: &[Keyframe], tick: f32, ty: Ty) -> (f32, Option<f32>) {
        let end_tick = a.end_tick.unwrap_or(a.length - 1.0);
        let loc = self.location(frames, tick, ty, pa_loop, a.length, lt.restart_tick(), a.is_easing_before());
        let k = &loc.frame;
        let mut start = self.rt.eval(&k.start);
        let mut end = self.rt.eval(&k.end);
        if matches!(ty, Ty::Rotation | Ty::Bend) {
            if !molang::any_constant(&k.start) {
                start = f64::from(start).to_radians() as f32;
            }
            if !molang::any_constant(&k.end) {
                end = f64::from(end).to_radians() as f32;
            }
        }
        let has_end = !a.loop_type.plays_again() && a.end_tick.is_some();
        let transition = if a.begin_tick.is_some_and(|b| b > tick) && !frames.is_empty() && loc.first {
            start = end;
            Some(k.length)
        } else if has_end && !frames.is_empty() && loc.last && end_tick <= tick {
            Some(a.length - end_tick)
        } else {
            None
        };
        let t = if k.length > 0.0 { loc.at / k.length } else { 0.0 };
        (k.easing.apply(&mut self.rt, start, end, k.length, t, &k.args), transition)
    }

    /// `getCurrentKeyFrameLocation`.
    #[allow(clippy::too_many_arguments)]
    fn location(&self, frames: &[Keyframe], age: f32, ty: Ty, pa_loop: bool, anim_time: f32, return_tick: f32, ease_before: bool) -> Location {
        if frames.is_empty() {
            let v = if ty == Ty::Scale { molang::constant(1.0) } else { Vec::new() };
            return Location { frame: Keyframe::new(0.0, v.clone(), v, Easing::Linear, Vec::new()), at: 0.0, first: false, last: false };
        }
        let first_frame = pa_loop.then(|| at_time(frames, return_tick));
        let mut first_end = 0.0;
        let loop_length = anim_time - return_tick;
        let mut total = 0.0f32;
        let n = frames.len();
        for (i, f) in frames.iter().enumerate() {
            total += f.length;
            if Some(i) == first_frame {
                first_end = total;
            }
            if total > age {
                if pa_loop && self.loop_started && Some(i) == first_frame {
                    return seam(frames, i, first_end + loop_length, age + loop_length, anim_time, ease_before);
                }
                return Location { frame: f.clone(), at: age - (total - f.length), first: i == 0, last: i == n - 1 };
            }
        }
        if let Some(ff) = first_frame {
            return seam(frames, ff, first_end + loop_length, age, anim_time, ease_before);
        }
        Location { frame: frames[n - 1].clone(), at: age, first: n == 1, last: true }
    }

    /// The pose this frame gives, while the emote is active.
    pub fn frame(&self) -> Option<Frame> {
        if !self.is_active() {
            return None;
        }
        let anim = self.current.clone()?;
        Some(Frame {
            anim,
            bones: self.bones.clone(),
            pivots: self.pivots.clone(),
            active: self.active.clone(),
            ticks: self.ticks(),
            torso_bend: self.torso_bend,
            torso_bend_sign: self.torso_bend_sign,
        })
    }
}

/// `Keyframe.getKeyframeAtTime`, as an index.
fn at_time(frames: &[Keyframe], tick: f32) -> usize {
    let mut total = 0.0;
    for (i, f) in frames.iter().enumerate() {
        total += f.length;
        if total > tick {
            return i;
        }
    }
    frames.len() - 1
}

/// `getLoopSeamLocation`: from the last keyframe back to the loop's first.
fn seam(frames: &[Keyframe], first: usize, end: f32, tick: f32, anim_time: f32, ease_before: bool) -> Location {
    let last = &frames[frames.len() - 1];
    let start = last_time(frames).min(anim_time);
    let easing = if ease_before { &frames[first] } else { last };
    Location {
        frame: Keyframe::new(end - start, last.end.clone(), frames[first].end.clone(), easing.easing, easing.args.clone()),
        at: tick - start,
        first: false,
        last: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built_in(i: usize) -> Arc<Animation> {
        Arc::new(crate::loader::load_json(crate::tests::BUILT_IN_JSON[i]).unwrap().remove(0))
    }

    #[test]
    fn euler_round_trip() {
        let mut m = Mat4::IDENTITY;
        m.rotate_zyx(0.3, -0.4, 0.5);
        let e = m.euler_zyx();
        assert!((e[0] - 0.5).abs() < 1e-5 && (e[1] + 0.4).abs() < 1e-5 && (e[2] - 0.3).abs() < 1e-5, "{e:?}");
    }

    #[test]
    fn plays_every_built_in_to_its_end() {
        for i in 0..crate::tests::BUILT_IN_JSON.len() {
            let a = built_in(i);
            let mut c = Controller::new();
            c.play(a.clone(), None, 0.0);
            let mut frames = 0;
            for _ in 0..400 {
                c.tick();
                if !c.process(0.5) {
                    break;
                }
                let f = c.frame().unwrap();
                for b in BONES {
                    let bone = f.bone(b);
                    assert!(bone.pos.iter().chain(&bone.rot).chain(&bone.scale).all(|v| v.is_finite()), "{} {b}: {bone:?}", a.name_or_id());
                }
                frames += 1;
            }
            if a.loop_type.plays_again() {
                assert!(c.is_active(), "{} stopped", a.name_or_id());
            } else {
                assert!(!c.is_active(), "{} never ended", a.name_or_id());
                assert!(frames as f32 <= a.length + 2.0);
            }
        }
    }

    /// The bones `c` gives after `ticks` whole ticks.
    fn at(i: usize, ticks: u32) -> Frame {
        let mut c = Controller::new();
        c.play(built_in(i), None, 0.0);
        for _ in 0..ticks {
            c.tick();
        }
        c.process(0.0);
        c.frame().unwrap()
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// Clap at tick 11, worked out by hand with PlayerAnimationLibrary's formulas: the right arm's
    /// pitch keyframes are -1.459968 at tick 10 (`EASEINCIRC`) and -1.484396 at tick 12; the
    /// legacy loader moves each easing to the next keyframe, so 10→12 eases in-circ, and half way
    /// in-circ is 1 - sqrt(0.75). The arm's `y: 2` is its default, so its offset is 0.
    #[test]
    fn clap_matches_the_library_at_tick_11() {
        let f = at(1, 11);
        let arm = f.bone("right_arm");
        let t = 1.0 - 0.75f32.sqrt();
        let pitch = -1.459_968 + (-1.484_396 - -1.459_968) * t;
        assert!(close(arm.rot[0], pitch), "{arm:?} vs {pitch}");
        assert!(close(arm.pos[1], 0.0), "{arm:?}");
        // Not in the emote: the body and the legs keep the game's pose.
        assert!(!f.active.contains("body") && !f.active.contains("right_leg"));
    }

    /// Waving at tick 24 (degrees, version 1, `torso` renamed `body`): the body's roll eases
    /// 8°→11° over ticks 10→46 out-expo (the easing of tick 10's keyframe), so at 14/36 it is
    /// 8 + 3·(1 - 2^(-10·14/36)); the right arm's roll eases 185°→95° over 20→25 in-out-sine
    /// (0.5·(1 - cos 0.8π) of the way); its yaw holds 90°.
    #[test]
    fn waving_matches_the_library_at_tick_24() {
        let f = at(0, 24);
        let body = f.bone("body");
        let roll = (8.0 + 3.0 * (1.0 - 2f32.powf(-10.0 * 14.0 / 36.0))).to_radians();
        assert!(close(body.rot[2], roll), "{body:?} vs {roll}");
        let arm = f.bone("right_arm");
        let k = 0.5 * (1.0 - (0.8 * std::f32::consts::PI).cos());
        let arm_roll = (185.0 - 90.0 * k).to_radians();
        assert!(close(arm.rot[2], arm_roll), "{arm:?} vs {arm_roll}");
        assert!(close(arm.rot[1], 90f32.to_radians()), "{arm:?}");
    }

    #[test]
    fn waving_raises_the_right_arm() {
        // waving: right arm roll to 185 degrees by tick 10.
        let a = built_in(0);
        let mut c = Controller::new();
        c.play(a, None, 0.0);
        for _ in 0..14 {
            c.tick();
        }
        c.process(0.0);
        let f = c.frame().unwrap();
        let arm = f.bone("right_arm");
        assert!(arm.rot[2].abs() > 1.5, "{arm:?}");
    }
}
