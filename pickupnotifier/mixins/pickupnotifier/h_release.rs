// A mouse button let go: taken when the config screen took its press.
fn pickupnotifier_release(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if crate::game::pickupnotifier_config::released(game, screen, button) {
        *canceled = true;
    }
}
