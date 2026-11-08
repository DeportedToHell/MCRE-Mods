// A key let go: nothing under the config screen hears it.
fn roughlyenoughitems_key_released(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::roughlyenoughitems_options::key_released(game, screen) {
        *canceled = true;
    }
}
