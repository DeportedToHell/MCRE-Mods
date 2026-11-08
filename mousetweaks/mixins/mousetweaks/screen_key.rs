// A key over the screen: Escape, Tab, the arrows, Enter and Space.
fn mousetweaks_screen_key(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::mousetweaks_screen::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
