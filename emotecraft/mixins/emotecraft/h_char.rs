// A character typed over Emotecraft's screens: the search field.
fn emotecraft_char(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, c: char, canceled: &mut bool) {
    if crate::game::emotecraft_ui::char_typed(game, screen, c) {
        *canceled = true;
    }
}
