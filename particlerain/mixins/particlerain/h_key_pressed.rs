// A key over the config screen.
fn particlerain_key_pressed(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::particlerain_screen::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
