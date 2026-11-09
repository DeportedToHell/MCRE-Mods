// The container screen's tooltip (`AbstractContainerScreen.extractTooltip`),
// drawn here with the lock key's hint enabled while it is drawn; kept where
// it was for the locked slot while the lock key is held. Spliced into the
// game's own, whose body it follows.
fn shulkerboxtooltip_slot_tooltip(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) -> core::ops::ControlFlow<()> {
    if !crate::game::shulkerboxtooltip_glue::slot_tooltip(game, batch) {
        let (x, y) = game.cursor;
        if let Some(slot) = game.slot_at(batch, x, y) {
            if let Some(ghost) = game.ghost_stack(slot) {
                game.draw_stack_tooltip(batch, &ghost, x, y);
            } else {
                let stack = game.slot_view(slot);
                if game.carried.is_none() || stack.as_ref().is_some_and(|s| game.tooltip_with_carried(s)) {
                    if let Some(stack) = stack {
                        game.draw_stack_tooltip(batch, &stack, x, y);
                    } else if let Some(text) = game.crafter_tooltip(slot) {
                        game.draw_tooltip_lines(batch, &[(horizon_client::text::Text::literal(text), u32::MAX)], x, y);
                    }
                }
            }
        }
    }
    crate::game::shulkerboxtooltip_glue::slot_tooltip_done();
    core::ops::ControlFlow::Break(())
}
