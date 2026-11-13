// No backpack into a shulker box through its side, by a hopper or a dropper
// (`ShulkerBoxBlockEntityMixin.canInsert`). (In the game's library: its own
// test for a backpack, every Traveler's Backpack backpack's id ending so.)
fn travelersbackpack_shulker_insert(view: &View, stack: &ItemStack) -> core::ops::ControlFlow<bool> {
    let backpack = stack.item.strip_prefix("travelersbackpack:").is_some_and(|n| {
        !n.ends_with("_upgrade") && !n.ends_with("sleeping_bag") && !["hose", "hose_nozzle", "backpack_tank"].contains(&n)
    });
    if view.kind == Kind::Shulker && backpack { core::ops::ControlFlow::Break(false) } else { core::ops::ControlFlow::Continue(()) }
}
