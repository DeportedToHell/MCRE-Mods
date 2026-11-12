// `ModItemGroups.createTabStack`: the creative tab's button is the standard
// backpack with its tanks full of water and lava. Horizon keeps only a tab
// icon's item id, so on the creative screen the bare standard backpack (no
// components: only the tab's button is drawn so) is drawn as that stack.
fn travelersbackpack_tab_icon(
    game: &crate::game::Game<'_>,
    batch: &mut HudBatch,
    stack: &ItemStack,
    x: f32,
    y: f32,
) -> core::ops::ControlFlow<()> {
    thread_local! {
        static ICON: Option<ItemStack> = horizon_client::item_data::parse_stack(
            "{count:1,id:\"travelersbackpack:standard\",components:{\"minecraft:custom_model_data\":{flags:[1b],strings:[\"creative_tab\"]}}}",
        )
        .ok()
        .flatten();
    }
    if stack.extra.is_some() || &*stack.item != "travelersbackpack:standard" || game.screen != Screen::Creative {
        return core::ops::ControlFlow::Continue(());
    }
    match ICON.with(Clone::clone) {
        Some(icon) => {
            game.draw_item(batch, &icon, x, y);
            core::ops::ControlFlow::Break(())
        }
        None => core::ops::ControlFlow::Continue(()),
    }
}
