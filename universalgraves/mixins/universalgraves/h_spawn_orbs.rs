// The experience of a death whose grave keeps it: no orbs.
fn universalgraves_spawn_orbs() -> core::ops::ControlFlow<()> {
    if crate::game::universalgraves::spawn_orbs() {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
