// A mouse button over a screen: the mods screen takes it while it is open.
fn modmenu_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::modmenu_glue::pressed(game, screen, button, modifiers) {
        *canceled = true;
    }
}
