// `AccessoriesMenu.quickMoveStack` from the inventory: armour to its empty
// armour slot, a shield-like item to an empty off hand, else the first
// accessory slot that takes it (`canMoveToAccessorySlot`), else between the
// inventory and the hotbar. Out of an accessory slot: vanilla's move.
fn accessories_quick_move(view: &mut View<'_>, i: usize, rules: &dyn Rules) -> core::ops::ControlFlow<()> {
    use core::ops::ControlFlow::{Break, Continue};
    use crate::lan::containers::accessories_menu as m;
    let Some(window) = view.guest.menu.as_ref().map(|m| m.window) else {
        return Continue(());
    };
    let conn = view.guest.conn;
    let Some(n) = m::own_slots(conn, window) else {
        return Continue(());
    };
    let size = match &view.container {
        Some(c) => c.len(),
        None => return Continue(()),
    };
    if i < size || !view.enabled(i) {
        return Continue(());
    }
    let Some(mut stack) = view.get(i) else {
        return Break(());
    };
    // Armour, then the off hand: their slots in the menu.
    let armor = rules.armor_index(&stack.item);
    let offhand = rules.equip_slot(&stack.item) == Some("offhand");
    for k in 0..n.min(size) {
        let Some(rule) = m::rule(conn, window, k) else {
            continue;
        };
        let fits = match rule.kind {
            m::Kind::Armor(a) => armor == Some(a),
            m::Kind::Offhand => offhand,
            m::Kind::Accessory => false,
        };
        if fits && view.enabled(k) && view.get(k).is_none() {
            let mut one = stack.clone();
            one.count = 1;
            view.set(k, Some(one));
            stack.count -= 1;
            view.set(i, (stack.count > 0).then_some(stack));
            return Break(());
        }
    }
    // The first accessory (not cosmetic) slot that is empty and takes it.
    for k in 0..n.min(size) {
        let Some(rule) = m::rule(conn, window, k) else {
            continue;
        };
        if rule.cosmetic || rule.kind != m::Kind::Accessory || !view.enabled(k) || view.get(k).is_some() || !m::valid(&rule, &stack, rules.items()) {
            continue;
        }
        let mut one = stack.clone();
        one.count = 1;
        view.set(k, Some(one));
        stack.count -= 1;
        view.set(i, (stack.count > 0).then_some(stack));
        return Break(());
    }
    // `moveItemStackTo` between the main inventory and the hotbar.
    let main = size..size + 27;
    let hotbar = size + 27..size + 36;
    let to = if main.contains(&i) { hotbar } else { main };
    let max = rules.max_stack(&stack.item).max(1);
    if max > 1 {
        for k in to.clone() {
            if stack.count == 0 {
                break;
            }
            if let Some(mut existing) = view.get(k)
                && existing.same_item_same_components(&stack)
                && existing.count < max
            {
                let moved = (max - existing.count).min(stack.count);
                existing.count += moved;
                stack.count -= moved;
                view.set(k, Some(existing));
            }
        }
    }
    for k in to {
        if stack.count == 0 {
            break;
        }
        if view.get(k).is_none() {
            view.set(k, Some(stack.clone()));
            stack.count = 0;
        }
    }
    view.set(i, (stack.count > 0).then_some(stack));
    Break(())
}
