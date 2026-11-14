// `AccessoriesBasedSlot.getMaxStackSize`: one item an accessory slot unless
// its `accessories:stack_settings` says more; an armour slot one; the off hand
// as vanilla's.
fn accessories_slot_max(view: &View<'_>, i: usize, stack: &ItemStack, rules: &dyn Rules) -> core::ops::ControlFlow<u32> {
    use crate::lan::containers::accessories_menu as m;
    let g = &*view.guest;
    let Some(window) = g.menu.as_ref().map(|m| m.window) else {
        return core::ops::ControlFlow::Continue(());
    };
    match m::rule(g.conn, window, i).map(|r| r.kind) {
        Some(m::Kind::Accessory) => core::ops::ControlFlow::Break(m::max_in_slot(stack, rules.items())),
        Some(m::Kind::Armor(_)) => core::ops::ControlFlow::Break(1),
        _ => core::ops::ControlFlow::Continue(()),
    }
}
