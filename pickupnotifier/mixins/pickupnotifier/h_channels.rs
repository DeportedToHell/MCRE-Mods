// The server half: a client registered or unregistered plugin channels, Pick
// Up Notifier's among them or not.
fn pickupnotifier_channels(
    game: &crate::game::Game<'_>,
    player: u128,
    channels: &[String],
    register: bool,
) {
    crate::game::pickupnotifier_glue::channels(game, player, channels, register);
}
