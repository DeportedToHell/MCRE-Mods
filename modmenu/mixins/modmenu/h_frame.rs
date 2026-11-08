// Once a frame: opens and closes the mods screen, runs what it asked for.
fn modmenu_frame(game: &mut crate::game::Game<'_>) {
    crate::game::modmenu_glue::frame(game);
}
