// The local player's input keys: InvMove's while a screen is open.
fn invmove_keys(keys: &mut crate::sprint::Keys) {
    if let Some(screen_keys) = crate::sprint::invmove_keys::take() {
        *keys = screen_keys;
    }
}
