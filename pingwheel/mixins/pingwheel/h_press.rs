// A mouse button down over the settings screen.
fn pingwheel_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::pingwheel_screen::pressed(game, screen, button, modifiers) {
        *canceled = true;
    }
}
