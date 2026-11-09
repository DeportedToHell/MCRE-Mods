// A mouse button let go over the config screen.
fn bwncr_release(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::bwncr_config::released(game, screen) {
        *canceled = true;
    }
}