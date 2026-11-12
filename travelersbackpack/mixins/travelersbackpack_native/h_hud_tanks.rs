// The worn backpack's tanks on the HUD, before the hotbar.
fn travelersbackpack_hud_tanks(game: &crate::game::Game<'_>, layer: &str, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::travelersbackpack_glue::hud_tanks(game, layer, batch);
}
