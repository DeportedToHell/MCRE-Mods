// A mouse button up over the screens.
fn plasmovoice_release(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if crate::game::plasmovoice_glue::released(game, screen, button) {
        *canceled = true;
    }
}
