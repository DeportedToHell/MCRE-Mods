// A key over the screen: the config screen's keys.
fn shulkerboxtooltip_key(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::shulkerboxtooltip_screen::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
