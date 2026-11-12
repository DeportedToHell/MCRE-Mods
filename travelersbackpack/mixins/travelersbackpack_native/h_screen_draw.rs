// The equip or unequip button.
fn travelersbackpack_screen_draw(
    game: &crate::game::Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
) {
    crate::game::travelersbackpack_glue::screen_draw(game, block, data, batch, origin);
}
