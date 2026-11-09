// A mouse button let go over the settings screen: a slider let go.
fn fallingleaves_release(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if crate::game::fallingleaves_screen::released(game, screen, button) {
        *canceled = true;
    }
}
