// The end of each server tick: changed ender chests go to their players.
fn shulkerboxtooltip_server_tick(game: &mut crate::game::Game<'_>, phase: &horizon_client::mod_api::ServerTickPhase) {
    if matches!(phase, horizon_client::mod_api::ServerTickPhase::ServerTickEnd { .. }) {
        crate::game::shulkerboxtooltip_glue::server_tick(game);
    }
}
