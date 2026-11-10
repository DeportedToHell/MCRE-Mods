// Over the open screen once it has drawn: the config screen.
fn particlerain_screen_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::particlerain_screen::render(game, batch, screen);
}
