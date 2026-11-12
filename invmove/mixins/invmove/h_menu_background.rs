// The in-world menu background: not drawn where InvMove hides the background.
fn invmove_menu_background(in_world: bool) -> core::ops::ControlFlow<()> {
    if in_world && crate::ui::invmove_background::hidden() {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
