// Right after the frame's dynamic lights are gathered (by Horizon's own
// extra or by a mod gathering them instead): the worn lanterns join them.
fn travelersbackpack_lantern_lights(game: &mut crate::game::Game<'_>, camera: glam::DVec3, partial: f32) {
    crate::game::travelersbackpack_glue::lantern_lights(game, camera, f64::from(partial));
}
