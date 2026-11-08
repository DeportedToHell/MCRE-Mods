// Horizon's dynamic lights for this frame: the mod gathers them while it is on.
fn dynamiclights_gather(
    game: &mut crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
) -> core::ops::ControlFlow<()> {
    if crate::game::dynamiclights_glue::gather(game, camera, alpha) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
