// Over a container screen: REI's overlay in place of the game's item panel.
fn roughlyenoughitems_draw(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) -> core::ops::ControlFlow<()> {
    if crate::game::roughlyenoughitems_view::draw(game, batch) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
