// A key let go over the config screen.
fn invmove_key_released(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::invmove_screen::key_released(game, screen) {
        *canceled = true;
    }
}
