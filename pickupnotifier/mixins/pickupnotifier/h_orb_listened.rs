// The server half hears experience orb pickups while a client takes them.
fn pickupnotifier_orb_listened(game: &crate::game::Game<'_>, listened: &mut bool) {
    crate::game::pickupnotifier_glue::orb_listened(game, listened);
}
