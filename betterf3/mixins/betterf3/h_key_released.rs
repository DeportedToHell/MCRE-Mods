fn betterf3_key_released(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::betterf3_screens::key_released(game, screen) {
        *canceled = true;
    }
}
