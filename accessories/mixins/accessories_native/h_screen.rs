// The accessories screen's size and slots.
fn accessories_screen(block: &str, screen: &mut Option<horizon_client::mod_api::NativeScreen>) {
    crate::game::accessories_glue::screen(block, screen);
}
