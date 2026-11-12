// `BackpackSlotItemHandler.isItemValid`: a backpack's storage (its menu's
// input slots) takes no backpack, and nothing that can't go into a container
// item (a shulker box).
fn travelersbackpack_may_place(view: &View<'_>, i: usize, stack: &ItemStack) -> core::ops::ControlFlow<bool> {
    let g = &*view.guest;
    let ours = g.menu.as_ref().is_some_and(|m| crate::lan::containers::travelersbackpack_menu::is_open(g.conn, m.window));
    let storage = matches!(&g.native_store, Some(NativeStore::Input(r)) if r.contains(&i));
    let item = &*stack.item;
    let backpack = item.strip_prefix("travelersbackpack:").is_some_and(|n| {
        !n.ends_with("_upgrade") && !n.ends_with("_sleeping_bag") && n != "backpack_tank" && n != "hose" && n != "hose_nozzle"
    });
    if ours && storage && (backpack || item.ends_with("shulker_box")) {
        return core::ops::ControlFlow::Break(false);
    }
    core::ops::ControlFlow::Continue(())
}
