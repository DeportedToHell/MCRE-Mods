// Over the open screen once it has drawn: the settings screen over the options.
fn fallingleaves_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::fallingleaves_screen::render(game, batch, screen);
}
