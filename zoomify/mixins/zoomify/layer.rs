// The HUD hides while the secondary zoom is on.
fn zoomify_layer(game: &crate::game::Game<'_>, canceled: &mut bool) {
    crate::game::zoomify_glue::gui_layer(game, canceled);
}
