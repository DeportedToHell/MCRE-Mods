// One of the game's tooltips starts: where its vertices begin.
fn roughlyenoughitems_tooltip_stack_begin(batch: &horizon_client::hud::HudBatch) {
    crate::game::roughlyenoughitems_defer::tooltip_begins(batch.vertices.len());
}
