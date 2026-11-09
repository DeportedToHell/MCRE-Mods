fn betterf3_release(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::betterf3_screens::released(game, screen) {
        *canceled = true;
    }
}
