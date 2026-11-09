// A block's ambient tick (`Block.animateTick`): Falling Leaves first, and for
// leaves in its place.
fn fallingleaves_leaf_tick(
    tables: &crate::animate_tick::AnimateTables,
    engine: &mut crate::particles::ParticleEngine,
    ctx: &crate::animate_tick::AnimateContext<'_>,
    state: horizon_world::registry::BlockStateId,
    pos: [i32; 3],
) -> core::ops::ControlFlow<()> {
    if crate::animate_tick::fallingleaves::block_tick(tables, engine, ctx, state, pos) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
