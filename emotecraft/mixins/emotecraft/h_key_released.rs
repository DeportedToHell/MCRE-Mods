// A key let go over Emotecraft's screens: the wheel closes on HOLD.
fn emotecraft_key_released(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, key: &str, canceled: &mut bool) {
    if crate::game::emotecraft_ui::key_released(game, screen, key) {
        *canceled = true;
    }
}
