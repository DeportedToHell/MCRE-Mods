// A title button drawn, on the startup screen too: the Mods button's icon and
// the mod count after the version.
fn modmenu_title_button(button: &crate::title::Button, batch: &mut crate::hud::HudBatch, atlas: &crate::hud::HudAtlas) {
    crate::title::modmenu_title::drawn(button, batch, atlas);
}
