// A mouse button let go: taken when the config screen or the overlays took its
// press.
fn roughlyenoughitems_release(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    let options = crate::game::roughlyenoughitems_options::released(game, screen, button);
    let overlay = crate::game::roughlyenoughitems_glue::released(game, screen, button);
    if options || overlay {
        *canceled = true;
    }
}
