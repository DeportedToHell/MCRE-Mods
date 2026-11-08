// A mouse button let go: the config screen's when it took the press.
fn smoothswapping_screen_release(button: i32, canceled: &mut bool) {
    if crate::game::smoothswapping_screen::released(button) {
        *canceled = true;
    }
}
