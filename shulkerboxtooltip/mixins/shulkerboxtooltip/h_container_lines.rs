// `ItemContainerContentsMixin`: no "N x item" lines unless the tooltip type
// is Vanilla.
fn shulkerboxtooltip_container_lines() -> core::ops::ControlFlow<Vec<String>> {
    if crate::item_tooltip::shulkerboxtooltip_flags::hide_container_lines() {
        return core::ops::ControlFlow::Break(Vec::new());
    }
    core::ops::ControlFlow::Continue(())
}
