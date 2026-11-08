// The cursor moved over a container screen, before the screen's own drag.
fn mousetweaks_drag(game: &mut crate::game::Game<'_>) {
    crate::game::mousetweaks_glue::drag(game);
}
