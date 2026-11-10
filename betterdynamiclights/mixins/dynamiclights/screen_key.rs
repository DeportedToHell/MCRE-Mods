// A key over the screen: the settings screen's keys.
fn dynamiclights_screen_key(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::dynamiclights_screen::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
