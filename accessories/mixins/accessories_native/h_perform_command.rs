// An /accessories command: run at the next tick.
fn accessories_perform_command(game: &mut crate::game::Game<'_>, player: &horizon_client::mod_api::NativeServerPlayer, command: &str, ran: &mut bool) {
    let _ = game;
    crate::game::accessories_glue::perform_command(player, command, ran);
}
