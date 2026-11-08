// A key let go: nothing under the config screen hears it.
fn pickupnotifier_key_released(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::pickupnotifier_config::key_released(game, screen) {
        *canceled = true;
    }
}
