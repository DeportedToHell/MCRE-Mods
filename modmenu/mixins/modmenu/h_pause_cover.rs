// The pause menu stays hidden under the mods screen.
fn modmenu_pause_cover() -> core::ops::ControlFlow<()> {
    if crate::game::modmenu::PAUSE_COVER.load(std::sync::atomic::Ordering::Relaxed) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
