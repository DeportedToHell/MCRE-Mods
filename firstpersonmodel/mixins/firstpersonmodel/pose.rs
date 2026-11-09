// First-person Model for Horizon: what the original's `HumanoidModelMixin`
// does at the end of `HumanoidModel.setupAnim` for the camera entity, here at
// the end of `PlayerModel.setupAnim`'s visibility (`player_model_visibility`),
// which every pose of the player's model goes through: the body, the armour
// copied from it, the cape, the ears and the held items' arms. Spliced into
// the client library's `mobs::anim` as `firstpersonmodel_pose`; the game sets
// the flags only while it draws the player's own body in first person.
use std::cell::Cell;

/// `LivingEntityRenderStateAccess` of the original: what to hide and how far
/// to turn the arms.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Flags {
    pub hide_left_arm: bool,
    pub hide_right_arm: bool,
    pub hide_body: bool,
    /// Added to both arms' x rotation (radians), the "Dynamic mode" tilt.
    pub arm_offset: f32,
}

thread_local! {
    static ACTIVE: Cell<Option<Flags>> = const { Cell::new(None) };
    /// The player's pitch while the vanilla hands follow the view (the mod
    /// on, the hands shown, Dynamic Mode applying), else none.
    static ARM_PITCH: Cell<Option<f32>> = const { Cell::new(None) };
}

/// Set once a frame, before the hands are drawn.
pub fn set_arm_pitch(pitch: Option<f32>) {
    ARM_PITCH.with(|a| a.set(pitch));
}

/// `HeldItemRendererMixin.modifyInverseArmHeight`: the hands' inverse arm
/// height (`equip_displacement`), raised while looking down.
pub fn arm_height(inverse_arm_height: f32) -> f32 {
    let Some(f) = ARM_PITCH.with(Cell::get) else {
        return inverse_arm_height;
    };
    if f > 15.0 {
        if f < 30.0 {
            return inverse_arm_height.max(1.0 - 15.0 / f);
        }
        if f < 35.0 {
            return inverse_arm_height.max(0.5 + ((f - 30.0) / 5.0) * 0.7);
        }
        return 1.2;
    }
    inverse_arm_height
}

/// The flags of the body being drawn now, or none for any other draw.
pub fn set(flags: Option<Flags>) {
    ACTIVE.with(|a| a.set(flags));
}

/// The own body in first person is being drawn.
pub fn active() -> bool {
    ACTIVE.with(Cell::get).is_some()
}

/// `HumanoidModelMixin.setupAnim` (TAIL): the head (and the hat on it) hidden,
/// then, unless the player is a spectator, the arms and the body as the flags
/// say, the arms turned by the offset.
pub fn apply(p: &mut super::Poses, spectator: bool) {
    let Some(f) = ACTIVE.with(Cell::get) else {
        return;
    };
    p.visible("head", false);
    p.visible("hat", false);
    if spectator {
        return;
    }
    p.visible("left_arm", !f.hide_left_arm);
    p.visible("right_arm", !f.hide_right_arm);
    if f.arm_offset != 0.0 {
        if p.has("left_arm") {
            p.get("left_arm")[3] += f.arm_offset;
        }
        if p.has("right_arm") {
            p.get("right_arm")[3] += f.arm_offset;
        }
    }
    p.visible("body", !f.hide_body);
}
