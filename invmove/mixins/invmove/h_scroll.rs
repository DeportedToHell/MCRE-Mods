// The wheel over the config screen: its list and its tab row.
fn invmove_scroll(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if crate::game::invmove_screen::scrolled(game, screen, scroll) {
        *canceled = true;
    }
}
