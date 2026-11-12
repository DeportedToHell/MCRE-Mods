// A placed backpack's upgrades at work.
fn travelersbackpack_block_entity_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut horizon_client::containers::ModBlockEntity,
) {
    crate::game::travelersbackpack_glue::block_entity_tick(game, pos, entity);
}
