// A key over the settings screen: the field, the focus, Escape.
fn pingwheel_key_pressed(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::pingwheel_screen::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
