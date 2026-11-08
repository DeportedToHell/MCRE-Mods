// A mouse button over the options: the config screen takes it while it is up.
fn clientsort_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::clientsort_config::pressed(game, screen, button, modifiers) {
        *canceled = true;
    }
}
