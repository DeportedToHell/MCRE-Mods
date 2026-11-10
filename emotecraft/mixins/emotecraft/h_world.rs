// The world opens or closes: the server half starts or stops.
fn emotecraft_world(game: &mut crate::game::Game<'_>, phase: horizon_client::mod_api::WorldPhase) {
    crate::game::emotecraft_server::world(game, phase);
}
