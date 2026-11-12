// `BackpackBaseMenu.moveItemStackTo`: a shift-click from the inventory into a
// backpack's storage merges onto its stacks, then takes the first empty slot
// that accepts it: a kept slot only its item, a slot sorting leaves alone
// (kept for nothing) none.
fn travelersbackpack_quick_move(view: &mut View<'_>, i: usize, rules: &dyn Rules) -> core::ops::ControlFlow<()> {
    use core::ops::ControlFlow::{Break, Continue};
    let Some(window) = view.guest.menu.as_ref().map(|m| m.window) else {
        return Continue(());
    };
    let Some(r) = crate::lan::containers::travelersbackpack_menu::move_rules(view.guest.conn, window) else {
        return Continue(());
    };
    let n = match &view.container {
        Some(c) => c.len(),
        None => return Continue(()),
    };
    if i < n || !view.enabled(i) {
        return Continue(());
    }
    let Some(mut stack) = view.get(i) else {
        return Continue(());
    };
    if stack.item.starts_with("travelersbackpack:") && stack.item.ends_with("_upgrade") {
        return Continue(());
    }
    let storage = r.storage.min(n);
    if rules.max_stack(&stack.item) > 1 {
        for k in 0..storage {
            if stack.count == 0 {
                break;
            }
            if !view.enabled(k) {
                continue;
            }
            if let Some(mut existing) = view.get(k)
                && existing.same_item_same_components(&stack)
            {
                let max = view.slot_max(k, &existing, rules);
                if existing.count < max {
                    let moved = (max - existing.count).min(stack.count);
                    existing.count += moved;
                    stack.count -= moved;
                    view.set(k, Some(existing));
                }
            }
        }
    }
    for k in 0..storage {
        if stack.count == 0 {
            break;
        }
        let kept = r.memory.iter().find(|m| m.0 == k);
        let accept = match kept {
            Some((_, item, true)) => item.same_item_same_components(&stack),
            Some((_, item, false)) => item.item == stack.item,
            None => !r.unsortable.contains(&k),
        };
        if accept && view.enabled(k) && view.get(k).is_none() && view.may_place(k, &stack, rules) {
            let moved = stack.count.min(view.slot_max(k, &stack, rules));
            view.set(k, Some(ItemStack { count: moved, ..stack.clone() }));
            stack.count -= moved;
        }
    }
    view.set(i, (stack.count > 0).then_some(stack));
    Break(())
}
