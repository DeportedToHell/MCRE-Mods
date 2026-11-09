// A key or mouse button down or up: Plasmo Voice's hotkeys and the menu key.
fn plasmovoice_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::plasmovoice_glue::key(game, key, pressed);
}
