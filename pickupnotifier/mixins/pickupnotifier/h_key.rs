// A key: the config screen takes every one.
fn pickupnotifier_key(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if !*canceled && crate::game::pickupnotifier_config::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
