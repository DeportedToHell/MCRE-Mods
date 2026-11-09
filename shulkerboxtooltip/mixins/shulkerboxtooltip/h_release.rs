// A mouse button let go: the config screen's when it took the press.
fn shulkerboxtooltip_release(button: i32, canceled: &mut bool) {
    if crate::game::shulkerboxtooltip_screen::released(button) {
        *canceled = true;
    }
}
