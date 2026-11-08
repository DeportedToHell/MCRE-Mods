// After each HUD layer: the list draws after `minecraft:effects`.
fn pickupnotifier_draw(
    game: &mut crate::game::Game<'_>,
    layer: &str,
    batch: &mut horizon_client::hud::HudBatch,
) {
    crate::game::pickupnotifier_glue::draw(game, layer, batch);
}
