// The spyglass overlay, shown or hidden as Zoomify says.
fn zoomify_spyglass(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) -> core::ops::ControlFlow<()> {
    if crate::game::zoomify_glue::spyglass(game, batch) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
