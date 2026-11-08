// The wheel: the cycle button under the pointer.
fn mousetweaks_screen_scroll(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if crate::game::mousetweaks_screen::scrolled(game, screen, scroll) {
        *canceled = true;
    }
}
