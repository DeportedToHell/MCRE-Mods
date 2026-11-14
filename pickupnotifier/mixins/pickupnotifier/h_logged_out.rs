// The server half: a player left; their client takes nothing more.
fn pickupnotifier_logged_out(player: u128) {
    crate::game::pickupnotifier_glue::logged_out(player);
}
