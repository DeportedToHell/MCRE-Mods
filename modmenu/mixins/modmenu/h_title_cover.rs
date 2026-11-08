// The title screen stays hidden under the mods screen.
fn modmenu_title_cover() -> core::ops::ControlFlow<()> {
    if crate::title::modmenu_title::COVER.load(std::sync::atomic::Ordering::Relaxed) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
