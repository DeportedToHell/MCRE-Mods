// A mouse button before the screen sees it: the buttons, and the middle
// button's sort.
fn clientsort_mouse(
    game: &mut crate::game::Game<'_>,
    button: i32,
    pressed: bool,
    modifiers: u32,
    canceled: &mut bool,
) {
    if game.native_on(crate::game::clientsort::ID) {
        crate::game::clientsort_glue::mouse(game, button, pressed, modifiers, canceled);
    }
}
