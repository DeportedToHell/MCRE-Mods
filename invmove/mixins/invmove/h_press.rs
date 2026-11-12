// A mouse button on a screen: a mod's text field may lose the keyboard; the
// config screen takes it while it is up.
fn invmove_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    crate::game::invmove_glue::focus_moved();
    if crate::game::invmove_screen::pressed(game, screen, button, modifiers) {
        *canceled = true;
    }
}
