// A character typed over the settings screen: its focused field's.
fn fallingleaves_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if crate::game::fallingleaves_screen::char_typed(game, screen, c) {
        *canceled = true;
    }
}
