// The pointer dragged with a button held: the scroll bars.
fn modmenu_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::modmenu_glue::dragged(game, screen) {
        *canceled = true;
    }
}
