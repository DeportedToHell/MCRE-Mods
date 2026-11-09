// The wheel over a screen.
fn firstpersonmodel_wheel(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if !*canceled && crate::game::firstpersonmodel_screen::scrolled(game, screen, scroll) {
        *canceled = true;
    }
}
