// The sponge backpack soaks up when a neighbour changes.
fn travelersbackpack_neighbor_changed(pos: horizon_core::position::BlockPos) {
    crate::game::travelersbackpack_glue::neighbor_changed(pos);
}
