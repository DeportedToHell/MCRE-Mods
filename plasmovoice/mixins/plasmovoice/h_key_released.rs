// A key let go over the screens: a hotkey being bound takes the keys pressed.
fn plasmovoice_key_released(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    canceled: &mut bool,
) {
    if crate::game::plasmovoice_glue::key_released(game, screen, key) {
        *canceled = true;
    }
}
