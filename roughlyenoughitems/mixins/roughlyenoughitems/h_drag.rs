// The pointer dragged with a button held: nothing under the config screen
// hears it.
fn roughlyenoughitems_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::roughlyenoughitems_options::dragged(game, screen) {
        *canceled = true;
    }
}
