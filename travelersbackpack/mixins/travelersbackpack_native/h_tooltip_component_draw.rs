// A backpack's contents under its tooltip while Ctrl is held: drawn.
fn travelersbackpack_tooltip_component_draw(
    game: &crate::game::Game<'_>,
    stack: &ItemStack,
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
) {
    crate::game::travelersbackpack_glue::tooltip_component_draw(game, stack, batch, origin);
}
