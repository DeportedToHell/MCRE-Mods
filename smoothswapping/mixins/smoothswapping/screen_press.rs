// A mouse button over the screen: the config screen's buttons.
fn smoothswapping_screen_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::smoothswapping_screen::pressed(game, screen, button, modifiers) {
        *canceled = true;
    }
}
