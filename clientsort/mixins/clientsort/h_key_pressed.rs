// A key over the config screen: its fields, a key being chosen, Escape.
fn clientsort_key_pressed(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if crate::game::clientsort_config::key_pressed(game, screen, key, modifiers) {
        *canceled = true;
    }
}
