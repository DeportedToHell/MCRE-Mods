// A character typed over the screens: the search and distance fields.
fn plasmovoice_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if crate::game::plasmovoice_glue::char_typed(game, screen, c) {
        *canceled = true;
    }
}
