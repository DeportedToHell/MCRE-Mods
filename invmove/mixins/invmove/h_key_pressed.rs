// A key on a screen: Escape, Enter and Tab may take the keyboard from a mod's
// text field; the config screen takes keys while it is up.
fn invmove_key_pressed(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if matches!(key, "key.keyboard.escape" | "key.keyboard.enter" | "key.keyboard.keypad.enter" | "key.keyboard.tab") {
        crate::game::invmove_glue::focus_moved();
    }
    if crate::game::invmove_screen::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
