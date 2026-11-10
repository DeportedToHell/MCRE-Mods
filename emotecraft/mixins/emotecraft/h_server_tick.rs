// The server's tick: the host's messages in, the server half's out.
fn emotecraft_server_tick(game: &mut crate::game::Game<'_>, phase: horizon_client::mod_api::ServerTickPhase) {
    crate::game::emotecraft_server::server_tick(game, phase);
}
