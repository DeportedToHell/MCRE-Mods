// A press on a screen: whether shift is held, for the backpack's buttons; a
// slot chosen with the void upgrade's trash bin instead of clicked; a tool
// chosen in the tools' radial menu.
fn travelersbackpack_screen_pressed(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    crate::game::travelersbackpack_glue::screen_mouse_pressed(modifiers);
    crate::game::travelersbackpack_glue::trash_press(screen, button, canceled);
    crate::game::travelersbackpack_glue::radial_press(game, button, canceled);
}
