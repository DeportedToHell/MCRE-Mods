// Over the Options screen: the config screen, while it is open on the mod's page.
fn pickupnotifier_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::pickupnotifier_config::render(game, batch, screen);
}
