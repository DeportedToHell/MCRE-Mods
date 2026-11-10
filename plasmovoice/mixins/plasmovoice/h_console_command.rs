// A command typed on the dedicated server's console: one of Plasmo Voice's runs here.
fn plasmovoice_console_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::plasmovoice_glue::console_command(game, command, ran);
}
