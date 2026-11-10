// The server's commands: /emotes.
fn emotecraft_server_commands(game: &mut crate::game::Game<'_>, commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    crate::game::emotecraft_server::register_commands(game, commands);
}
