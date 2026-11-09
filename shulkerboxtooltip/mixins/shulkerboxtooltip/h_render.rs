// Over the Options screen: the config screen while it is open.
fn shulkerboxtooltip_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::shulkerboxtooltip_screen::render(game, batch, screen);
}
