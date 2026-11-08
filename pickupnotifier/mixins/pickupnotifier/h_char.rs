// A character: the config screen's focused box.
fn pickupnotifier_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if !*canceled && crate::game::pickupnotifier_config::char_typed(game, screen, c) {
        *canceled = true;
    }
}
