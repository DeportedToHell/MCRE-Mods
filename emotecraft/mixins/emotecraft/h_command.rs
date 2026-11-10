// A client command: /emotes-client play|stop.
fn emotecraft_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::emotecraft_glue::command(game, command, ran);
}
