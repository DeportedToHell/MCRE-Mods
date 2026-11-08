// The pointer dragged with a button held: the config screen's.
fn pickupnotifier_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::pickupnotifier_config::dragged(game, screen) {
        *canceled = true;
    }
}
