// Once a frame: the voice chat's packets out, where the players are in, the config, the screens.
fn plasmovoice_frame(game: &mut crate::game::Game<'_>) {
    crate::game::plasmovoice_glue::frame(game);
}
