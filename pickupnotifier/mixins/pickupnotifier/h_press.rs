// A mouse button over the Options screen: the config screen's.
fn pickupnotifier_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if !*canceled && crate::game::pickupnotifier_config::pressed(game, screen, button) {
        *canceled = true;
    }
}
