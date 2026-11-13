// The creeper backpack: a death turned away (`DeathHandler`, `creeperAbility`):
// the wearer stays at 1 health and is saved at the tick.
fn travelersbackpack_guest_die(host: &mut LanHost, gi: usize) -> core::ops::ControlFlow<()> {
    let Some(g) = host.hub.guests.get_mut(gi) else {
        return core::ops::ControlFlow::Continue(());
    };
    if g.health <= 0.0 && !g.dead && crate::game::travelersbackpack_glue::creeper_spares(g.entity_id) {
        g.health = 1.0;
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
