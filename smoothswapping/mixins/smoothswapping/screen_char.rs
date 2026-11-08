// A character typed: nothing under the config screen hears it.
fn smoothswapping_screen_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::smoothswapping_screen::swallowed(game, screen) {
        *canceled = true;
    }
}
