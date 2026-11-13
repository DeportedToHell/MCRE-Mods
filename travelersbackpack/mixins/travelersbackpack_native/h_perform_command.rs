// `/tb ...` run by an operator.
fn travelersbackpack_perform_command(
    game: &mut crate::game::Game<'_>,
    player: &horizon_client::mod_api::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::travelersbackpack_glue::perform_command(game, player, command, ran);
}
