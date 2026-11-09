// The boss bar and the sidebar hidden while the debug screen shows.
fn betterf3_layer(game: &mut crate::game::Game<'_>, layer: &str, canceled: &mut bool) {
    if crate::game::betterf3_overlay::hides_layer(game, layer) {
        *canceled = true;
    }
}
