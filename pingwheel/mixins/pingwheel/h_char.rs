// A character typed into the settings screen's channel field.
fn pingwheel_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if crate::game::pingwheel_screen::char_typed(game, screen, c) {
        *canceled = true;
    }
}
