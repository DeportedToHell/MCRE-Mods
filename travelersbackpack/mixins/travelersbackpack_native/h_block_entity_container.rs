// A placed backpack's slots.
fn travelersbackpack_block_entity_container(block: &str, container: &mut Option<horizon_client::containers::ModContainer>) {
    crate::game::travelersbackpack_glue::block_entity_container(block, container);
}
