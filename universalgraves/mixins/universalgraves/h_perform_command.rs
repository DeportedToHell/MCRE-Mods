// `/graves`, run at the tick.
fn universalgraves_perform_command(player: &horizon_client::mod_api::NativeServerPlayer, command: &str, ran: &mut bool) {
    crate::game::universalgraves::perform_command(player, command, ran);
}
