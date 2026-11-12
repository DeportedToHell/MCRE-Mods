// Over the open backpack screen, items included: the void upgrade's trash bin.
fn travelersbackpack_screen_render_post(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
    crate::game::travelersbackpack_glue::screen_render_post(game, batch, screen);
}
