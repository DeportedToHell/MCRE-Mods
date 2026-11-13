// The key that opens the accessories screen.
fn accessories_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::accessories_glue::key(game, key, pressed);
}
