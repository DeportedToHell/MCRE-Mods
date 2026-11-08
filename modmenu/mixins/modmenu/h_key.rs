// A key went down or up: Open Mod Menu.
fn modmenu_key(game: &crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::modmenu_glue::key(game, key, pressed);
}
