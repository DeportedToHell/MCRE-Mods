// `SleepingBagBlock` is a `BedBlock`: a guest asleep in a sleeping bag stays
// asleep (Horizon's `checkBedExists` knows a bed by its `_bed` name). The
// same tick as for a bed, with the bed there.
fn travelersbackpack_guest_sleep(
    game: &mut crate::game::Game<'_>,
    host: &mut crate::game::lan::LanHost,
    gi: usize,
) -> core::ops::ControlFlow<()> {
    use core::ops::ControlFlow::{Break, Continue};
    let Some(head) = host.hub.guests.get(gi).and_then(|g| g.bed) else {
        return Continue(());
    };
    let block = game.world.block(head[0], head[1], head[2]);
    let name = &game.res.facts().get(block).name;
    if !(name.starts_with("travelersbackpack:") && name.ends_with("_sleeping_bag")) {
        return Continue(());
    }
    let g = &mut host.hub.guests[gi];
    let was_deep = g.sleep_timer >= 100;
    g.sleep_timer = crate::game::tick_sleep_timer(g.sleep_timer, true);
    let identity = (!was_deep && g.sleep_timer >= 100).then(|| (g.uuid, g.name.clone()));
    if g.dead || g.health <= 0.0 {
        game.wake_guest(host, gi);
    } else {
        g.pitch = 0.0;
        if let Some(m) = g.motion.as_mut() {
            m.pitch = 0.0;
        }
    }
    if let Some((player, name)) = identity {
        game.native_player_deep_sleep(player, &name);
    }
    Break(())
}
