// Each frame: the zoom moves on.
fn zoomify_frame(game: &mut crate::game::Game<'_>, elapsed: std::time::Duration) {
    crate::game::zoomify_glue::frame(game, elapsed);
}
