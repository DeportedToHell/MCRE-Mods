// The end of the model's setupAnim: the emote's bones on the player's parts and cape.
fn emotecraft_setup(value: (), m: &crate::mobs::Mob, s: &State, p: &mut Poses) {
    crate::mobs::anim::emotecraft_pose::apply(m, s, p);
    value
}
