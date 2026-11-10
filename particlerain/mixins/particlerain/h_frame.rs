// Each frame: the settings, the mod's data files, the screens' requests.
fn particlerain_frame(game: &mut crate::game::Game<'_>) {
    crate::game::particlerain_glue::frame(game);
    crate::game::particlerain_screen::frame(game);
}
