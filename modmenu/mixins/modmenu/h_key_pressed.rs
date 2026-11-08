// A key over a screen: the search box, the list's arrows, Escape.
fn modmenu_key_pressed(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::modmenu_glue::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
