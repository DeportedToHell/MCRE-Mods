// A mouse button over BetterF3's screens.
fn betterf3_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::betterf3_screens::pressed(game, screen, button, modifiers) {
        *canceled = true;
    }
}
