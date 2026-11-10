// A key let go over the settings screen.
fn pingwheel_key_released(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::pingwheel_screen::key_released(game, screen) {
        *canceled = true;
    }
}
