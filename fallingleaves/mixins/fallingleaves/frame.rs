// Each frame: the settings, the mod's data files, the settings screen's requests.
fn fallingleaves_frame(game: &mut crate::game::Game<'_>) {
    crate::game::fallingleaves_glue::frame(game);
    crate::game::fallingleaves_screen::frame(game);
}
