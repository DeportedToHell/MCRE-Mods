// The wheel: Toggle Mod under the pointer.
fn smoothswapping_screen_scroll(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if crate::game::smoothswapping_screen::scrolled(game, screen, scroll) {
        *canceled = true;
    }
}
