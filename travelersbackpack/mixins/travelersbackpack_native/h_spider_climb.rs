// `spiderAbility`: the spider backpack's wearer, against a wall and out of
// water, climbs it as a ladder (up at 0.2, held while sneaking).
fn travelersbackpack_spider_climb(world: &mut MotionWorld<'_, '_, '_>) -> core::ops::ControlFlow<Result<bool>> {
    if crate::motion::travelersbackpack_spider::climbs()
        && world.player.collision.horizontal
        && !world.player.flying
        && !world.player.fluids.was_touching_water
    {
        world.player.on_climbable = true;
        return core::ops::ControlFlow::Break(Ok(true));
    }
    core::ops::ControlFlow::Continue(())
}
