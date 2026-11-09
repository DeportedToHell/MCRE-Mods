// A key over a screen.
fn firstpersonmodel_key_pressed(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if !*canceled && crate::game::firstpersonmodel_screen::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
