// A mouse button let go over the config screen.
fn invmove_release(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if crate::game::invmove_screen::released(game, screen, button) {
        *canceled = true;
    }
}
