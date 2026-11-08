// An item tooltip: the mod's name at its foot.
fn roughlyenoughitems_tooltip(
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(horizon_client::text::Text, u32)>,
) {
    crate::game::roughlyenoughitems_glue::tooltip(stack, lines);
}
