// The pointer dragged: nothing under the options screen hears it.
fn mousetweaks_screen_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::mousetweaks_screen::dragged(game, screen) {
        *canceled = true;
    }
}
