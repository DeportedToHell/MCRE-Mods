// A dedicated server's console command: /emotes runs here.
fn emotecraft_console_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::emotecraft_server::console_command(game, command, ran);
}
