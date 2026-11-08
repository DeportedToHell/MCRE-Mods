// The pointer dragged: the config screen's slider or curve point.
fn smoothswapping_screen_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if crate::game::smoothswapping_screen::dragged(game, screen, button) {
        *canceled = true;
    }
}
