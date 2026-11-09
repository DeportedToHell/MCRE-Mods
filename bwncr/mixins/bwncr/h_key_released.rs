// A key let go over the config screen.
fn bwncr_key_released(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::bwncr_config::key_released(game, screen) {
        *canceled = true;
    }
}