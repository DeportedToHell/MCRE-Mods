// Enter or Space on the title screen's focused Mods button.
fn modmenu_title_confirm(title: &mut crate::title::Title) -> core::ops::ControlFlow<(Action, bool)> {
    if crate::title::modmenu_title::confirmed(title) {
        return core::ops::ControlFlow::Break((crate::title::Action::None, true));
    }
    core::ops::ControlFlow::Continue(())
}
