// A mouse button down over Emotecraft's screens.
fn emotecraft_press(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    if crate::game::emotecraft_ui::pressed(game, screen, button) {
        *canceled = true;
    }
}
