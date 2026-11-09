fn betterf3_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if crate::game::betterf3_screens::char_typed(game, screen, c) {
        *canceled = true;
    }
}
