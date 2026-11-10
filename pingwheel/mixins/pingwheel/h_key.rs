// A key or mouse button down or up: Ping Location and Open Settings.
fn pingwheel_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::pingwheel_glue::key(game, key, pressed);
}
