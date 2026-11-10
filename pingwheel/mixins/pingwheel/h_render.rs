// Over the open screen once it has drawn: the settings screen.
fn pingwheel_render(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
    crate::game::pingwheel_glue::render(game, batch, screen);
}
