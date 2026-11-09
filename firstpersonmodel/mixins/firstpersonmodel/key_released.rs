// A key let go over a screen.
fn firstpersonmodel_key_released(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if !*canceled && crate::game::firstpersonmodel_screen::key_released(game, screen) {
        *canceled = true;
    }
}
