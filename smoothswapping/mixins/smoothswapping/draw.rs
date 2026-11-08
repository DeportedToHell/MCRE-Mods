// Over the slots, under the held stack and the tooltips: the flying stacks.
fn smoothswapping_draw(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch) {
    crate::game::smoothswapping_glue::draw(game, batch);
}
