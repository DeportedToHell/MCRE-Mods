// Over the open screen once it has drawn: the settings screen, or the Mod Menu screen.
fn plasmovoice_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::plasmovoice_glue::render(game, batch, screen);
}
