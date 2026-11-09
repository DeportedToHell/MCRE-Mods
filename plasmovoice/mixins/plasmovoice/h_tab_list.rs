// The player list: with Plasmo Voice's icons when they show (a Horizon addition).
fn plasmovoice_tab_list(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) -> core::ops::ControlFlow<()> {
    if crate::game::plasmovoice_tab::draw(game, batch) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
