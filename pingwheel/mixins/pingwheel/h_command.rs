// /pingwheel: help, config, channel.
fn pingwheel_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::pingwheel_glue::command(game, command, ran);
}
