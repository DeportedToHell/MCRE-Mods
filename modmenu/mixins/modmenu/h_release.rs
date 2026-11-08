// A mouse button let go.
fn modmenu_release(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if crate::game::modmenu_glue::released(game, screen, button) {
        *canceled = true;
    }
}
