// The end of each client tick: the list's time runs, unless the game is paused.
fn pickupnotifier_tick(game: &mut crate::game::Game<'_>) {
    crate::game::pickupnotifier_glue::tick(game);
}
