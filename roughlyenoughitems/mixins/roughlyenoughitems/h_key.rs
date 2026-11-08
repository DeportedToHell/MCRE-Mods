// A key over a screen: the search box, R, U, A and the page keys.
fn roughlyenoughitems_key(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if !*canceled && crate::game::roughlyenoughitems_glue::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
