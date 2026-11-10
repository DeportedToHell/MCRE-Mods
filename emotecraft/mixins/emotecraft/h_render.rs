// Over the open screen: Emotecraft's screens.
fn emotecraft_render(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
    crate::game::emotecraft_ui::render(game, batch, screen);
}
