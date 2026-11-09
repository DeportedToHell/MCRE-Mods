// An item's tooltip lines: the container's line and the key hints.
fn shulkerboxtooltip_tooltip(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(horizon_client::text::Text, u32)>,
) {
    crate::game::shulkerboxtooltip_glue::tooltip(game, stack, lines);
}
