// The pointer dragged: a slider or the scrollbar of the settings screen.
fn dynamiclights_screen_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::dynamiclights_screen::dragged(game, screen) {
        *canceled = true;
    }
}
