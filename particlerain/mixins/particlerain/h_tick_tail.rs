// After the particle engine's tick (the client's particle tick goes on with the item pickups):
// Particle Rain's tick.
fn particlerain_tick_tail(game: &mut crate::game::Game<'_>) {
    crate::game::particlerain_glue::tick_tail(game);
}
