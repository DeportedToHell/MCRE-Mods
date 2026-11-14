// An item's tooltip: the accessory slots it fits and what it gives there.
fn accessories_tooltip(game: &crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, lines: &mut Vec<(horizon_client::text::Text, u32)>) {
    crate::game::accessories_glue::tooltip(game, stack, lines);
}
