// The wheel over the accessories grid.
fn accessories_screen_scrolled(screen: &horizon_client::mod_api::NativeScreenView<'_>, scroll: [f64; 2], modifiers: u32, canceled: &mut bool) {
    let _ = modifiers;
    crate::game::accessories_glue::screen_scrolled(screen, scroll, canceled);
}
