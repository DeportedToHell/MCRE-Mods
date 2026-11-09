// A mouse button down over the screens: theirs while they are up.
fn plasmovoice_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::plasmovoice_glue::pressed(game, screen, button, modifiers) {
        *canceled = true;
    }
}
