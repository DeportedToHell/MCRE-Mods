// A character typed: the config screen's search field.
fn zoomify_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if !*canceled && crate::game::zoomify_screen::char_typed(game, screen, c) {
        *canceled = true;
    }
}
