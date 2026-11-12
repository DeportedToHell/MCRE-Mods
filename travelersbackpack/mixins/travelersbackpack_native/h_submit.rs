// A placed backpack drawn: its tanks' fluid.
fn travelersbackpack_submit(
    game: &crate::game::Game<'_>,
    state: horizon_world::registry::BlockStateId,
    entity: &horizon_client::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    crate::game::travelersbackpack_glue::block_entity_submit(game, state, entity, draw);
}
