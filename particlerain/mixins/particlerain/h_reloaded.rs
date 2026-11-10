// The resources were reloaded: the particles again.
fn particlerain_reloaded(game: &mut crate::game::Game<'_>) {
    crate::game::particlerain_glue::reloaded(game);
}
