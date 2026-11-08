// A mouse button over a screen: the config screen's.
fn zoomify_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    if !*canceled && crate::game::zoomify_screen::pressed(game, screen, button, modifiers) {
        *canceled = true;
    }
}
