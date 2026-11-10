// The wheel over the settings screen.
fn pingwheel_scroll(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if crate::game::pingwheel_screen::scrolled(game, screen, scroll) {
        *canceled = true;
    }
}
