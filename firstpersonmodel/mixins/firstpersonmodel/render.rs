// Over the Options screen: the config screen, while it is open on the mod's page.
fn firstpersonmodel_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::firstpersonmodel_screen::render(game, batch, screen);
}
