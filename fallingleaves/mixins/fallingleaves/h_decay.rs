// A leaf block decays on the single player's server: its leaves fall on the client.
fn fallingleaves_decay(level: &crate::game::GameLevel<'_>, pos: horizon_core::position::BlockPos) {
    let state = level.world.block(pos.x, pos.y, pos.z);
    horizon_client::animate_tick::fallingleaves::push_decayed(state, [pos.x, pos.y, pos.z]);
}
