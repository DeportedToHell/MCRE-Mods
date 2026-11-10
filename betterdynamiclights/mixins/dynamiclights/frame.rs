// Each frame: the toggle key's presses; the settings screen's clicks,
// key binding, closing and data files.
fn dynamiclights_frame(game: &mut crate::game::Game<'_>) {
    crate::game::dynamiclights_glue::frame(game);
    crate::game::dynamiclights_screen::frame(game);
}
