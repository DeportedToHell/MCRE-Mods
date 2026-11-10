// A player's chat box asks the server to complete /emotes play's emote.
fn emotecraft_complete(game: &mut crate::game::Game<'_>, player: u128, label: &str, arguments: &[String], range: [usize; 2], suggestions: &mut Vec<horizon_client::mod_api::Suggestion>) {
    crate::game::emotecraft_server::complete(game, player, label, arguments, range, suggestions);
}
