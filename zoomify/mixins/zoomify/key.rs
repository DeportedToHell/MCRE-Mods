// A key went down or up: the zoom keys.
fn zoomify_key(game: &crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::zoomify_glue::key(game, key, pressed);
}
