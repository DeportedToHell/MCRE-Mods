// A mouse button over the screen: the settings screen's widgets.
fn dynamiclights_screen_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if crate::game::dynamiclights_screen::pressed(game, screen, button) {
        *canceled = true;
    }
}
