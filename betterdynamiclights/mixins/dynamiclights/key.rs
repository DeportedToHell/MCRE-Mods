// A key went down or up: the toggle key.
fn dynamiclights_key(game: &crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::dynamiclights_glue::key(game, key, pressed);
}
