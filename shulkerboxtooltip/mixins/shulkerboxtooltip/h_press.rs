// A mouse button over the screen: the config screen's widgets.
fn shulkerboxtooltip_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if crate::game::shulkerboxtooltip_screen::pressed(game, screen, button) {
        *canceled = true;
    }
}
