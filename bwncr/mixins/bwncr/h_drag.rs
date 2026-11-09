// The config screen's scroll bar dragged.
fn bwncr_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::bwncr_config::dragged(game, screen) {
        *canceled = true;
    }
}