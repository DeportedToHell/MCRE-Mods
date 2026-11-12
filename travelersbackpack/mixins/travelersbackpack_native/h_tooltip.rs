// The hose's mode and tank in its tooltip.
fn travelersbackpack_tooltip(game: &crate::game::Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    crate::game::travelersbackpack_glue::tooltip(game, stack, lines);
}
