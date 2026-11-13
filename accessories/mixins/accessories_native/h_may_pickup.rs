// `AccessoriesBasedSlot.mayPickup`: a worn accessory with the curse of
// binding stays on a survival player (`canUnequip`); cosmetics come off.
fn accessories_may_pickup(view: &View<'_>, i: usize, rules: &dyn Rules) -> core::ops::ControlFlow<bool> {
    let _ = rules;
    let g = &*view.guest;
    let Some(window) = g.menu.as_ref().map(|m| m.window) else {
        return core::ops::ControlFlow::Continue(());
    };
    let Some(rule) = crate::lan::containers::accessories_menu::rule(g.conn, window, i) else {
        return core::ops::ControlFlow::Continue(());
    };
    let bound = view.get(i).is_some_and(|s| s.enchantment_level("minecraft:binding_curse") > 0);
    core::ops::ControlFlow::Break(rule.cosmetic || rule.kind == crate::lan::containers::accessories_menu::Kind::Offhand || g.creative() || !bound)
}
