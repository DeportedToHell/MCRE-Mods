// After the whole GUI: with F1, the icons over heads that always show.
fn plasmovoice_gui_post(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch, gui_hidden: bool) {
    crate::game::plasmovoice_glue::gui_post(game, batch, gui_hidden);
}
