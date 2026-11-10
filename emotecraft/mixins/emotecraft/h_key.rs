// A key or mouse button: the wheel key, the stop key, the emotes' own keys.
fn emotecraft_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::emotecraft_glue::key(game, key, pressed);
}
