// The HUD: the ping markers and the arrows at the screen's edge.
fn pingwheel_hud(game: &mut crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::pingwheel_glue::draw_hud(game, batch);
}
