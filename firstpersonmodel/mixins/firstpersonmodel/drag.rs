// The pointer dragged over a screen.
fn firstpersonmodel_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if !*canceled && crate::game::firstpersonmodel_screen::dragged(game, screen) {
        *canceled = true;
    }
}
