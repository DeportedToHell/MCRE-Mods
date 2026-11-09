// The head of the client's particle tick: the wind, and the leaves asked for.
fn fallingleaves_tick(game: &mut crate::game::Game<'_>) {
    crate::game::fallingleaves_glue::tick(game);
}
