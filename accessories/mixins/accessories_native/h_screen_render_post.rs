// Over any screen: the inventory's button to the accessories.
fn accessories_screen_render_post(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
    crate::game::accessories_glue::screen_render_post(game, batch, screen);
}
