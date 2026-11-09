// The wheel over a bundle whose preview shows: through all its items, in the
// compact preview's order when that one shows (`BundleMouseActionsMixin`).
// Spliced into the game's own bundle wheel, whose helpers it uses.
fn shulkerboxtooltip_bundle_scroll(game: &mut crate::game::Game<'_>, x: f64, y: f64) -> core::ops::ControlFlow<bool> {
    let Some(slot) = game.inventory_target().0 else {
        return core::ops::ControlFlow::Continue(());
    };
    if !game.bundle_slot(slot) {
        return core::ops::ControlFlow::Continue(());
    }
    let Some(bundle) = game.slot_mut(slot).clone() else {
        return core::ops::ControlFlow::Continue(());
    };
    let Some((count, order)) = crate::game::shulkerboxtooltip_glue::bundle_wheel(game, &bundle) else {
        return core::ops::ControlFlow::Continue(());
    };
    if count == 0 {
        return core::ops::ControlFlow::Continue(());
    }
    let (wheel_x, wheel_y) = game.bundle_mouse.wheel.on_mouse_scroll(x, y);
    let wheel = if wheel_y == 0 { -wheel_x } else { wheel_y };
    if wheel != 0 {
        let selected = bundle.data().map_or(-1, |d| d.bundle_selected.0);
        let next = match order {
            None => horizon_client::mouse::ScrollWheelHandler::next_selection(f64::from(wheel), selected, count as i32),
            Some(order) => {
                let current = usize::try_from(selected).ok().and_then(|s| order.iter().position(|r| *r == s));
                let virtual_next = match current {
                    Some(v) => horizon_client::mouse::ScrollWheelHandler::next_selection(f64::from(wheel), v as i32, order.len() as i32),
                    None => 0,
                };
                order.get(virtual_next.max(0) as usize).map_or(selected, |r| *r as i32)
            }
        };
        if next != selected {
            game.select_bundle(slot, next);
        }
    }
    core::ops::ControlFlow::Break(true)
}
