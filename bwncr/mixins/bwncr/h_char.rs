// A character typed into the config screen's fields.
fn bwncr_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if crate::game::bwncr_config::char_typed(game, screen, c) {
        *canceled = true;
    }
}