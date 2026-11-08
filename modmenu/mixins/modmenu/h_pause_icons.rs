// The pause menu's icon row, moved for the Mods icon.
fn modmenu_pause_icons(
    icons: [(f32, f32, &'static str); 4],
    w: f32,
    h: f32,
    links: bool,
    additions: bool,
    split: bool,
) -> [(f32, f32, &'static str); 4] {
    crate::game::modmenu::pause_icons(icons, w, h, u8::from(links || additions) + u8::from(split))
}
