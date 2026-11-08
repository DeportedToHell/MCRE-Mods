// Each frame: its time, for the flights; the config screen's clicks,
// closing and data files.
fn smoothswapping_frame(game: &mut crate::game::Game<'_>, elapsed: std::time::Duration) {
    crate::game::smoothswapping_glue::frame(elapsed);
    crate::game::smoothswapping_screen::frame(game);
}
