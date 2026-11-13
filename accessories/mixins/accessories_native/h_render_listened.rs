// Players' models are drawn with the mod's layer.
fn accessories_render_listened(listened: &mut bool) {
    crate::game::accessories_glue::render_listened(listened);
}
