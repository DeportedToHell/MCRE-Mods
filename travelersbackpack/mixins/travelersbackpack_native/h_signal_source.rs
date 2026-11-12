// The redstone backpack is a signal source.
fn travelersbackpack_signal_source(state: horizon_world::registry::BlockStateId, source: &mut bool) {
    crate::game::travelersbackpack_glue::is_signal_source(state, source);
}
