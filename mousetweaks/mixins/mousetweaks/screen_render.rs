// Over the Options screen: Mouse Tweaks' options screen while it is open.
fn mousetweaks_screen_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::mousetweaks_screen::render(game, batch, screen);
}
