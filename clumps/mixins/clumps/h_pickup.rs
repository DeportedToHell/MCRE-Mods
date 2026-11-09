// A player takes an orb: the whole of it at once (`playerTouch`).
fn clumps_pickup(game: &crate::game::Game<'_>, value: &mut u32, count: &mut u32) {
    crate::game::clumps_glue::pickup(game, value, count);
}
