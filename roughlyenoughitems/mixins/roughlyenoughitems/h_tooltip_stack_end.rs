// One of the game's tooltips is drawn: where its vertices end.
fn roughlyenoughitems_tooltip_stack_end(value: (), batch: &horizon_client::hud::HudBatch) {
    crate::game::roughlyenoughitems_defer::tooltip_ends(batch.vertices.len());
    value
}
