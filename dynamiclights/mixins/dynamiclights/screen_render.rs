// Over the Options screen: the settings screen while it is open.
fn dynamiclights_screen_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::dynamiclights_screen::render(game, batch, screen);
}
