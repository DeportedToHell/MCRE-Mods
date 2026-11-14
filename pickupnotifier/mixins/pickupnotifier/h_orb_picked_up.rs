// The server half: a player is about to take an experience orb; their client
// is told its value, in case it never had the orb.
fn pickupnotifier_orb_picked_up(game: &mut crate::game::Game<'_>, player: u128, orb: u32) {
    crate::game::pickupnotifier_glue::orb_picked_up(game, player, orb);
}
