// Each frame: the background, the mod's data files, the config screen's requests.
fn invmove_frame(game: &mut crate::game::Game<'_>) {
    crate::game::invmove_glue::frame(game);
    crate::game::invmove_screen::frame(game);
}
