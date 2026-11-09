// A tooltip text is drawn, the last of a box: where its vertices end.
fn roughlyenoughitems_tooltip_text(value: (), batch: &horizon_client::hud::HudBatch) {
    crate::game::roughlyenoughitems_defer::tooltip_ends(batch.vertices.len());
    value
}
