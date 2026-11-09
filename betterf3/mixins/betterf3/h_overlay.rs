// BetterF3's debug screen in place of the game's F3 text while it shows.
fn betterf3_overlay(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    size: [u32; 2],
) -> core::ops::ControlFlow<()> {
    if crate::game::betterf3_overlay::draw(game, batch, size) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
