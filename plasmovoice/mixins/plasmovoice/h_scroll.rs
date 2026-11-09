// The wheel over the screens.
fn plasmovoice_scroll(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if crate::game::plasmovoice_glue::scrolled(game, screen, scroll) {
        *canceled = true;
    }
}
