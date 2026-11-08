// The pause menu's buttons with the Mods button.
fn modmenu_pause_buttons(
    buttons: Vec<(f32, f32, f32, &'static str, u8, bool)>,
    w: f32,
    h: f32,
    links: bool,
    additions: bool,
    split: bool,
) -> Vec<(f32, f32, f32, &'static str, u8, bool)> {
    crate::game::modmenu::pause_buttons(buttons, w, h, u8::from(links || additions) + u8::from(split))
}
