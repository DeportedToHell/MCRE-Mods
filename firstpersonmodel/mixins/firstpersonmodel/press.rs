// A mouse button over a screen: the config screen's.
fn firstpersonmodel_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if !*canceled && crate::game::firstpersonmodel_screen::pressed(game, screen, button) {
        *canceled = true;
    }
}
