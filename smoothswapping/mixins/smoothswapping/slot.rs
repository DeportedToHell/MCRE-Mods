// A slot's item: a destination shows only what has landed.
fn smoothswapping_slot(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    slot: usize,
    stack: &Option<horizon_client::items::ItemStack>,
    x: f32,
    y: f32,
) -> core::ops::ControlFlow<()> {
    if crate::game::smoothswapping_glue::slot(game, batch, slot, stack, x, y) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
