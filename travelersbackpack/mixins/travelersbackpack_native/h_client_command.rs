// `/tb_client hide|show` (`BackpackIconCommands`).
fn travelersbackpack_client_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::travelersbackpack_glue::client_command(game, command, ran);
}
