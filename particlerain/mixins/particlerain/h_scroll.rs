// The wheel over the config screen: its list scrolls.
fn particlerain_scroll(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if crate::game::particlerain_screen::scrolled(game, screen, scroll) {
        *canceled = true;
    }
}
