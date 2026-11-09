// A character typed over a screen.
fn firstpersonmodel_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if !*canceled && crate::game::firstpersonmodel_screen::char_typed(game, screen, c) {
        *canceled = true;
    }
}
