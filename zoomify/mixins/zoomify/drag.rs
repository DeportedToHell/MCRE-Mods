// The pointer dragged with a button held: the config screen's sliders,
// scrollbar and search field.
fn zoomify_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::zoomify_screen::dragged(game, screen) {
        *canceled = true;
    }
}
