// /bwncr listen.
fn bwncr_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::bwncr_glue::command(game, command, ran);
}