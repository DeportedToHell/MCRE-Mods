// Over the title screen and the pause menu: the mods screen, the mod count
// and the Mods icon buttons.
fn modmenu_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::modmenu_view::render(game, batch, screen);
}
