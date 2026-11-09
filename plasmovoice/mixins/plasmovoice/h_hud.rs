// The HUD: the activation icon and the list of who is talking.
fn plasmovoice_hud(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::plasmovoice_glue::draw_hud(game, batch);
}
