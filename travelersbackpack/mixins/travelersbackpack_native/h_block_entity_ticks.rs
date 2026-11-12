// A placed backpack's block entity ticks.
fn travelersbackpack_block_entity_ticks(block: &str, ticks: &mut bool) {
    crate::game::travelersbackpack_glue::block_entity_ticks(block, ticks);
}
