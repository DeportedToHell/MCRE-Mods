// `SleepingBagBlock`: sleeping in a sleeping bag sets the respawn point only
// with `enableSleepingBagSpawnPoint` (off by default).
fn travelersbackpack_bag_spawn(game: &crate::game::Game<'_>, state: horizon_world::registry::BlockStateId) -> core::ops::ControlFlow<bool> {
    if crate::game::travelersbackpack_glue::is_bag_state(game, state) {
        core::ops::ControlFlow::Break(crate::game::travelersbackpack_config::config().enable_sleeping_bag_spawn_point)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
