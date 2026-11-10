// The model's root transform: the emote's body bone moves the whole player.
fn emotecraft_base(m: &Mob, alpha: f32, origin: glam::Mat4) -> core::ops::ControlFlow<glam::Mat4> {
    crate::mobs::anim::emotecraft_pose::base_head(m, alpha, origin)
}
