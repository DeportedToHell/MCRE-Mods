// The wheel: the config screen's list scrolls.
fn shulkerboxtooltip_scroll(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if crate::game::shulkerboxtooltip_screen::scrolled(game, screen, scroll) {
        *canceled = true;
    }
}
