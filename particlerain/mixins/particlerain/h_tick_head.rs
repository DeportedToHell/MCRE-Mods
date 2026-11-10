// The head of the client's particle tick: the game's own weather splashes and rain sound held back.
fn particlerain_tick_head(game: &mut crate::game::Game<'_>) {
    crate::game::particlerain_glue::tick_head(game);
}
