// Over the open screen once it has drawn: the config screen over the options.
fn invmove_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::invmove_screen::render(game, batch, screen);
}
