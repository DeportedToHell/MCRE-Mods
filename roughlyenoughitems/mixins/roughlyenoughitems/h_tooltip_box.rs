// A tooltip box starts: where its vertices begin.
fn roughlyenoughitems_tooltip_box(batch: &horizon_client::hud::HudBatch) {
    crate::game::roughlyenoughitems_defer::tooltip_begins(batch.vertices.len());
}
