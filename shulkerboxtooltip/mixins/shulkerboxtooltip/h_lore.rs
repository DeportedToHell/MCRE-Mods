// `ItemStackMixin.removeLore`: a shulker box's lore goes while "Hide Shulker
// Box Lore" is on.
fn shulkerboxtooltip_lore(stack: &crate::items::ItemStack) -> core::ops::ControlFlow<Vec<crate::text::Text>> {
    if crate::item_tooltip::shulkerboxtooltip_flags::hide_lore_of(&stack.item) {
        return core::ops::ControlFlow::Break(Vec::new());
    }
    core::ops::ControlFlow::Continue(())
}
