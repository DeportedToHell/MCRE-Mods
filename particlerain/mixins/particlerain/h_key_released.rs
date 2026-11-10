// A key let go over the config screen.
fn particlerain_key_released(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, canceled: &mut bool) {
    if crate::game::particlerain_screen::key_released(game, screen) {
        *canceled = true;
    }
}
