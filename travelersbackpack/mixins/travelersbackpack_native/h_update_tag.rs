// What clients get of a placed backpack: its tanks' fluids.
fn travelersbackpack_update_tag(
    entity: &horizon_client::containers::ModBlockEntity,
    tag: &mut Option<horizon_world::nbt::Compound>,
) {
    crate::game::travelersbackpack_glue::block_entity_update_tag(entity, tag);
}
