// The wheel over the config screen.
fn bwncr_scroll(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if crate::game::bwncr_config::scrolled(game, screen, scroll) {
        *canceled = true;
    }
}