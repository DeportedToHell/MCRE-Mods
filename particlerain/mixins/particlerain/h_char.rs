// A character typed into the config screen.
fn particlerain_char(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, c: char, canceled: &mut bool) {
    if crate::game::particlerain_screen::char_typed(game, screen, c) {
        *canceled = true;
    }
}
