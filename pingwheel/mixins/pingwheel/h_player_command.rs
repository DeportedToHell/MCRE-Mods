// A command a player typed: /pingwheel:server.
fn pingwheel_player_command(game: &mut crate::game::Game<'_>, player: u128, command: &str, canceled: &mut bool, replies: &mut Vec<horizon_client::text::Text>) {
    crate::game::pingwheel_glue::player_command(game, player, command, canceled, replies);
}
