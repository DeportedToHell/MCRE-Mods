// The world opens or closes: the voice server starts or stops.
fn plasmovoice_world(game: &mut crate::game::Game<'_>, phase: horizon_client::mod_api::WorldPhase) {
    crate::game::plasmovoice_glue::world(game, phase);
}
