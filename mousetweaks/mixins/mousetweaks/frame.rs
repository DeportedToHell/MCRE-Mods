// Each frame: the tweaks forget a screen that closed.
fn mousetweaks_frame(game: &mut crate::game::Game<'_>) {
    crate::game::mousetweaks_glue::frame(game);
}
