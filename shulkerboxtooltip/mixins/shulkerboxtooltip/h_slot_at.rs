// While a tooltip is locked, its slot is the one under the pointer
// (`isHovering`), wherever the pointer is.
fn shulkerboxtooltip_slot_at() -> core::ops::ControlFlow<Option<usize>> {
    match crate::game::shulkerboxtooltip_glue::locked_slot() {
        Some(slot) => core::ops::ControlFlow::Break(Some(slot)),
        None => core::ops::ControlFlow::Continue(()),
    }
}
