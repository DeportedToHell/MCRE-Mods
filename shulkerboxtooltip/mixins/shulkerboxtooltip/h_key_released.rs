// A key let go: nothing under the config screen hears it.
fn shulkerboxtooltip_key_released(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::shulkerboxtooltip_screen::swallowed(game, screen) {
        *canceled = true;
    }
}
