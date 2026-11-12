// The redstone backpack gives a full signal.
fn travelersbackpack_block_signal(pos: horizon_core::position::BlockPos, signal: &mut i32) {
    crate::game::travelersbackpack_glue::block_signal(pos, signal);
}
