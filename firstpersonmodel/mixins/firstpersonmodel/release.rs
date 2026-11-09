// A mouse button let go over a screen.
fn firstpersonmodel_release(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if !*canceled && crate::game::firstpersonmodel_screen::released(game, screen, button) {
        *canceled = true;
    }
}
