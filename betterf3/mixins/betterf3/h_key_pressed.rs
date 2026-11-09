fn betterf3_key_pressed(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::betterf3_screens::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
