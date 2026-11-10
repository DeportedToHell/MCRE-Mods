// The pointer dragged over the config screen: a slider, the scrollbar, a field's selection.
fn particlerain_drag(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, canceled: &mut bool) {
    if crate::game::particlerain_screen::dragged(game, screen) {
        *canceled = true;
    }
}
