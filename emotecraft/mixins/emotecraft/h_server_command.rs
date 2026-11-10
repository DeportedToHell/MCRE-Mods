// A player's command: /emotes runs here.
fn emotecraft_server_command(game: &mut crate::game::Game<'_>, player: &horizon_client::mod_api::NativeServerPlayer, command: &str, ran: &mut bool) {
    crate::game::emotecraft_server::command(game, player, command, ran);
}
