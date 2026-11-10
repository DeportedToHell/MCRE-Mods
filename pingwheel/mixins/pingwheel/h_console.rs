// A line typed on the dedicated server's console: /pingwheel:server.
fn pingwheel_console(game: &mut crate::game::Game<'_>, line: &str, cancel: &mut bool, replies: &mut Vec<horizon_client::text::Text>) {
    crate::game::pingwheel_glue::console_line(game, line, cancel, replies);
}
