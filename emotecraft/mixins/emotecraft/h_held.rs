// Where a held item is drawn: the emote's item bones move it.
fn emotecraft_held(m: &Mob, alpha: f32, camera: glam::DVec3, data: &MobData, left: bool) -> core::ops::ControlFlow<Option<glam::Mat4>> {
    crate::mobs::anim::emotecraft_pose::held_head(m, alpha, camera, data, left)
}
