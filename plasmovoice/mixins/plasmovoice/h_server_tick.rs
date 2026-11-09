// The server's tick: where the players are, and the voice server's packets out.
fn plasmovoice_server_tick(game: &mut crate::game::Game<'_>, phase: horizon_client::mod_api::ServerTickPhase) {
    crate::game::plasmovoice_glue::server_tick(game, phase);
}
