// A player's command: one of Plasmo Voice's runs here.
fn plasmovoice_command(game: &mut crate::game::Game<'_>, player: &horizon_client::mod_api::NativeServerPlayer, command: &str, ran: &mut bool) {
    crate::game::plasmovoice_glue::command(game, player, command, ran);
}
