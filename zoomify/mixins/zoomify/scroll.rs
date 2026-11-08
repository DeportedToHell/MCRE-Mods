// The wheel while playing: it zooms while zooming.
fn zoomify_scroll(game: &crate::game::Game<'_>, accumulated: [i32; 2], canceled: &mut bool) {
    crate::game::zoomify_glue::scrolling(game, accumulated, canceled);
}
