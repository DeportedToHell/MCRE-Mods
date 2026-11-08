// The hand's field of view, divided by the zoom too.
fn zoomify_hand(value: f32, game: &crate::game::Game<'_>) -> f32 {
    crate::game::zoomify_glue::hand_fov(game, value)
}
