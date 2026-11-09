fn betterf3_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::betterf3_screens::dragged(game, screen) {
        *canceled = true;
    }
}
