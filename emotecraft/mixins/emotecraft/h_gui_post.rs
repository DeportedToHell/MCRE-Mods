// After the whole GUI: Emotecraft's toasts.
fn emotecraft_gui_post(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::emotecraft_ui::draw_toasts(game, batch);
}
