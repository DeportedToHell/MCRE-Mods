// A mouse button let go: the options screen's when it took the press.
fn mousetweaks_screen_release(button: i32, canceled: &mut bool) {
    if crate::game::mousetweaks_screen::released(button) {
        *canceled = true;
    }
}
