// `AccessoriesBasedSlot.getMaxStackSize`: one item an accessory slot (an
// accessory with no `stack_settings` component).
fn accessories_slot_max(view: &View<'_>, i: usize, stack: &ItemStack, rules: &dyn Rules) -> core::ops::ControlFlow<u32> {
    let _ = (stack, rules);
    let g = &*view.guest;
    let Some(window) = g.menu.as_ref().map(|m| m.window) else {
        return core::ops::ControlFlow::Continue(());
    };
    if crate::lan::containers::accessories_menu::own_slots(g.conn, window).is_some_and(|n| i < n) {
        return core::ops::ControlFlow::Break(1);
    }
    core::ops::ControlFlow::Continue(())
}
