// A placed backpack's block entity.
fn travelersbackpack_block_entity(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::travelersbackpack_glue::block_entity(block, entity);
}
