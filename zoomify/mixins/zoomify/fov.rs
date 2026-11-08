// The world's field of view (`Camera.calculateFov`'s result), divided by the
// zoom, below the 1° the native hook would take.
fn zoomify_fov(value: f32, game: &crate::game::Game<'_>) -> f32 {
    crate::game::zoomify_glue::fov(game, value)
}
