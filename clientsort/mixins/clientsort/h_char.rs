// A character typed over the config screen: its focused field's.
fn clientsort_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if crate::game::clientsort_config::char_typed(game, screen, c) {
        *canceled = true;
    }
}
