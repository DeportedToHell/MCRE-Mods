// The pointer dragged over the settings screen: a slider, the field's selection.
fn pingwheel_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::pingwheel_screen::dragged(game, screen) {
        *canceled = true;
    }
}
