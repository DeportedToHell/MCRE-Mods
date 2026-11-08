// What the mouse reads: sensitivity, cinematic camera and spyglass slowdown.
fn zoomify_look(look: LookOptions, game: &crate::game::Game<'_>) -> LookOptions {
    crate::game::zoomify_glue::look(game, look)
}
