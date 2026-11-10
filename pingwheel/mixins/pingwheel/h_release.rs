// A mouse button up over the settings screen.
fn pingwheel_release(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if crate::game::pingwheel_screen::released(game, screen, button) {
        *canceled = true;
    }
}
