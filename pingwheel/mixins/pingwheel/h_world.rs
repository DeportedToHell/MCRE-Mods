// The world opens or closes: the server half's config and channels.
fn pingwheel_world(game: &mut crate::game::Game<'_>, phase: horizon_client::mod_api::WorldPhase) {
    crate::game::pingwheel_glue::world(game, phase);
}
