// A mouse button let go: taken when the overlays took its press.
fn roughlyenoughitems_release(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    if crate::game::roughlyenoughitems_glue::released(game, screen, button) {
        *canceled = true;
    }
}
