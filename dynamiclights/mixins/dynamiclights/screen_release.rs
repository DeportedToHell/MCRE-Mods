// A mouse button let go: the settings screen's when it took the press.
fn dynamiclights_screen_release(button: i32, canceled: &mut bool) {
    if crate::game::dynamiclights_screen::released(button) {
        *canceled = true;
    }
}
