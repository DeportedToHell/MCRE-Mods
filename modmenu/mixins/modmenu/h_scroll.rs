// The wheel: the mod list, the description and the options.
fn modmenu_scroll(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if crate::game::modmenu_glue::scrolled(game, screen, scroll) {
        *canceled = true;
    }
}
