// The Mods button clicked on the title screen.
fn modmenu_title_click(
    title: &mut crate::title::Title,
    x: f32,
    y: f32,
    size: (f32, f32),
) -> core::ops::ControlFlow<(Action, bool)> {
    if crate::title::modmenu_title::clicked(title, x, y, size) {
        return core::ops::ControlFlow::Break((crate::title::Action::None, true));
    }
    core::ops::ControlFlow::Continue(())
}
