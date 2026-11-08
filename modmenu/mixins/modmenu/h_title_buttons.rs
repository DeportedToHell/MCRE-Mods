// The main title screen's buttons with the Mods button.
fn modmenu_title_buttons(
    title: &crate::title::Title,
    w: f32,
    h: f32,
) -> core::ops::ControlFlow<Vec<Button>> {
    match crate::title::modmenu_title::buttons(title, w, h) {
        Some(buttons) => core::ops::ControlFlow::Break(buttons),
        None => core::ops::ControlFlow::Continue(()),
    }
}
