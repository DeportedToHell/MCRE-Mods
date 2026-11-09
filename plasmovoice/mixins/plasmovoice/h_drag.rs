// The pointer dragged over the screens: sliders, scrollbars.
fn plasmovoice_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::plasmovoice_glue::dragged(game, screen) {
        *canceled = true;
    }
}
