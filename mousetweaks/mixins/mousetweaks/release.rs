// A button let go over a container screen.
fn mousetweaks_release(game: &mut crate::game::Game<'_>, right: bool) {
    crate::game::mousetweaks_glue::release(game, right);
}
