// The pointer dragged: the config screen's scrollbar or field.
fn shulkerboxtooltip_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::shulkerboxtooltip_screen::dragged(game, screen) {
        *canceled = true;
    }
}
