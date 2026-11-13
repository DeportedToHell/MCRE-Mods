// `AccessoriesBasedSlot.mayPlace`: an accessories menu's own slots take what
// their slot type's validators allow.
fn accessories_may_place(view: &View<'_>, i: usize, stack: &ItemStack, rules: &dyn Rules) -> core::ops::ControlFlow<bool> {
    use crate::lan::containers::accessories_menu as m;
    let g = &*view.guest;
    let Some(window) = g.menu.as_ref().map(|m| m.window) else {
        return core::ops::ControlFlow::Continue(());
    };
    match m::rule(g.conn, window, i) {
        Some(rule) => core::ops::ControlFlow::Break(m::valid(&rule, stack, rules.items())),
        None => core::ops::ControlFlow::Continue(()),
    }
}
