// Before a HUD layer: the icons over heads go under the hotbar.
fn plasmovoice_layer(game: &mut crate::game::Game<'_>, layer: &str, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::plasmovoice_glue::layer_pre(game, layer, batch);
}
