// The config screen over the options.
fn bwncr_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::bwncr_config::render(game, batch, screen);
}