// Over the Options screen: REI's config screen, while it is open on REI's page.
fn roughlyenoughitems_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::roughlyenoughitems_options::render(game, batch, screen);
}
