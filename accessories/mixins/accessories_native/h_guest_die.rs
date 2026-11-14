// `checkTotemDeathProtection` with a totem in the accessory slots: the blow
// that would kill the player leaves them at 1 health; the totem is used up at the tick.
fn accessories_guest_die(host: &mut LanHost, gi: usize) -> core::ops::ControlFlow<()> {
    let Some(g) = host.hub.guests.get_mut(gi) else {
        return core::ops::ControlFlow::Continue(());
    };
    if g.health <= 0.0 && !g.dead && crate::game::accessories_glue::totem_in_slots(g.saved.as_ref()) {
        g.health = 1.0;
        crate::game::accessories_glue::totem_saved(g.uuid);
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
