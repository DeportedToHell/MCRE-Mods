// The parts the own body hides in first person, and the arms' tilt.
fn firstpersonmodel_parts(value: (), p: &mut Poses, spectator: bool) {
    crate::mobs::anim::firstpersonmodel_pose::apply(p, spectator);
    value
}
