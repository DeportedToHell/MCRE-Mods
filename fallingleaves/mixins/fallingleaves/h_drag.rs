// The pointer dragged with a button held: the settings screen's scroll bar or slider.
fn fallingleaves_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::fallingleaves_screen::dragged(game, screen) {
        *canceled = true;
    }
}
