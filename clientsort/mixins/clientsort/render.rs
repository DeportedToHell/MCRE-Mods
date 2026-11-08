// Over the open screen once it has drawn: the trigger buttons.
fn clientsort_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    if game.native_on(crate::game::clientsort::ID) {
        crate::game::clientsort_glue::render(game, batch, screen);
    }
}
