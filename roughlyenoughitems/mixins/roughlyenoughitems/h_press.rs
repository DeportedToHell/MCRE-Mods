// A mouse button over a screen: the config screen's over the Options screen,
// the overlays' clicks.
fn roughlyenoughitems_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    if !*canceled
        && (crate::game::roughlyenoughitems_options::pressed(game, screen, button)
            || crate::game::roughlyenoughitems_glue::pressed(game, screen, button, modifiers))
    {
        *canceled = true;
    }
}
