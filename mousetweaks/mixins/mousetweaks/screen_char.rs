// A character typed: nothing under the options screen hears it.
fn mousetweaks_screen_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::mousetweaks_screen::swallowed(game, screen) {
        *canceled = true;
    }
}
