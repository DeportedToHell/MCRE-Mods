// The pointer dragged over Emotecraft's screens: sliders, the list's scrollbar.
fn emotecraft_drag(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, canceled: &mut bool) {
    if crate::game::emotecraft_ui::dragged(game, screen) {
        *canceled = true;
    }
}
