// A character typed: the search box's.
fn modmenu_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if crate::game::modmenu_glue::char_typed(game, screen, c) {
        *canceled = true;
    }
}
