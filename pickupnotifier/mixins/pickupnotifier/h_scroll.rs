// The wheel: the config screen's list.
fn pickupnotifier_scroll(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if !*canceled && crate::game::pickupnotifier_config::scrolled(game, screen, scroll) {
        *canceled = true;
    }
}
