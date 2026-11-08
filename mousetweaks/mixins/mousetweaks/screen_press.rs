// A mouse button over the screen: the options screen's buttons.
fn mousetweaks_screen_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::mousetweaks_screen::pressed(game, screen, button, modifiers) {
        *canceled = true;
    }
}
