// The server asks the pickup hook while the mod is on.
fn clumps_pickup_listened(game: &crate::game::Game<'_>, listened: &mut bool) {
    crate::game::clumps_glue::pickup_listened(game, listened);
}
