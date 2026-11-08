// A key let go: nothing under the settings screen hears it.
fn dynamiclights_screen_key_released(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::dynamiclights_screen::swallowed(game, screen) {
        *canceled = true;
    }
}
