// Where Horizon draws its own sort buttons (after the slots, before the
// held stack and the tooltips): ClientSort's buttons.
fn clientsort_draw(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::clientsort_glue::draw(game, batch);
}
