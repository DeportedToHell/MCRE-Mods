// A press: the inventory's button, the accessories screen's buttons and toggles.
fn accessories_screen_pressed(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, modifiers: u32, canceled: &mut bool) {
    let _ = modifiers;
    crate::game::accessories_glue::screen_pressed(game, screen, button, canceled);
}
