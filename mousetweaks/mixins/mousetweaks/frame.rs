// Each frame: the tweaks forget a screen that closed; the options screen's
// clicks, closing and data files.
fn mousetweaks_frame(game: &mut crate::game::Game<'_>) {
    crate::game::mousetweaks_glue::frame(game);
    crate::game::mousetweaks_screen::frame(game);
}
