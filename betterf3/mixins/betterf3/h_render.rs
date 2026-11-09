// The config screens over the options (or as the mod's own screen).
fn betterf3_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::betterf3_screens::render(game, batch, screen);
}
