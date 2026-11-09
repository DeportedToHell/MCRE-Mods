// An item's tooltip while its preview shows: the box sized for the preview
// inside it, or left as it is when the preview sits outside, then the
// preview (`PreviewClientTooltipComponent`). Spliced into the game's own
// tooltip drawing, whose helpers it uses.
fn shulkerboxtooltip_stack_tooltip(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    stack: &horizon_client::items::ItemStack,
    creative: bool,
    x: f32,
    y: f32,
    after_name: &Vec<(horizon_client::text::Text, u32)>,
) -> core::ops::ControlFlow<()> {
    let Some(preview) = crate::game::shulkerboxtooltip_glue::preview_for(game, stack) else {
        return core::ops::ControlFlow::Continue(());
    };
    let mut cancel = false;
    game.native_tooltip_render_pre(stack, batch, x, y, &mut cancel);
    if cancel {
        return core::ops::ControlFlow::Break(());
    }
    let mut lines = game.stack_tooltip_lines(stack, false, creative);
    if lines.is_empty() {
        return core::ops::ControlFlow::Break(());
    }
    lines.splice(1..1, after_name.iter().cloned());
    let lines = crate::game::Game::resolved(&lines);
    // Vanilla's heights: the lines 10 each with 2 after the first, then the
    // component; Horizon's geometry counts a lone line 8 high.
    let n = lines.len() as f32;
    let (min_width, extra) = match preview.inside_size() {
        Some((w, h)) => (w, h + if lines.len() == 1 { 2.0 } else { 0.0 }),
        None => (0.0, 0.0),
    };
    let (tx, ty, w, h) =
        crate::game::Game::tooltip_geometry(&game.gui.atlas, &lines, (x, y), (batch.width, batch.height), [min_width, extra]);
    game.tooltip_box(batch, Some(stack), tx, ty, w, h);
    crate::game::Game::tooltip_text(batch, &game.gui.atlas, &lines, tx, ty);
    crate::game::shulkerboxtooltip_glue::draw_preview(game, batch, preview, [tx, ty, w, h], ty + n * 10.0 + 2.0);
    core::ops::ControlFlow::Break(())
}
