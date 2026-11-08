// /pickupnotifier: the settings page, and the hidden items.
fn pickupnotifier_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::pickupnotifier_glue::command(game, command, ran);
}
