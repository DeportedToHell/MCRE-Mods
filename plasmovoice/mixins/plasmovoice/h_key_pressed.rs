// A key over the screens: fields, hotkeys being bound, the menu key, Escape.
fn plasmovoice_key_pressed(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::plasmovoice_glue::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
