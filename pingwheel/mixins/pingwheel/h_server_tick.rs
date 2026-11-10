// The server's tick: the players' operator levels, for /pingwheel:server.
fn pingwheel_server_tick(game: &mut crate::game::Game<'_>, phase: horizon_client::mod_api::ServerTickPhase) {
    crate::game::pingwheel_glue::server_tick(game, phase);
}
