// Over the Options screen: Zoomify's config screen, while it is open on its page.
fn zoomify_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::zoomify_screen::render(game, batch, screen);
}
