// `ModItemGroups`: the mod's creative tab hands out backpacks as the original
// does: the standard one with tanks and then at each tier, every design with
// its tanks upgrade put in (`STARTER_UPGRADES`).
fn travelersbackpack_creative_tabs(plan: &Plan<'_>, doc: &mut J) -> core::ops::ControlFlow<Result<()>> {
    thread_local! {
        static INSIDE: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
    }
    if INSIDE.with(core::cell::Cell::get) {
        return core::ops::ControlFlow::Continue(());
    }
    INSIDE.with(|c| c.set(true));
    let result = plan.creative_tabs(doc);
    INSIDE.with(|c| c.set(false));
    if result.is_ok()
        && let Some(J::Arr(tabs)) = doc.at("tabs")
    {
        for tab in tabs.iter_mut() {
            if !matches!(tab.get("id"), Some(J::Str(id)) if id == "travelersbackpack:travelers_backpack") {
                continue;
            }
            // The tab's button, the standard backpack with tanks (`createTabStack`).
            tab.set(
                "icon",
                J::Str("{count:1,id:\"travelersbackpack:standard\",components:{\"minecraft:custom_model_data\":{flags:[1b],strings:[\"creative_tab\"]}}}".into()),
            );
            let Some(J::Arr(items)) = tab.get_mut("items") else {
                continue;
            };
            let mut standard = 0usize;
            for item in items.iter_mut() {
                let J::Str(snbt) = item else {
                    continue;
                };
                let Some(id) = snbt.split("id:\"").nth(1).and_then(|r| r.split('"').next()).map(str::to_owned) else {
                    continue;
                };
                let Some(name) = id.strip_prefix("travelersbackpack:") else {
                    continue;
                };
                let backpack = !name.ends_with("_upgrade")
                    && !name.ends_with("_sleeping_bag")
                    && !["backpack_tank", "hose", "hose_nozzle"].contains(&name);
                if !backpack {
                    continue;
                }
                let tier = if name == "standard" {
                    standard += 1;
                    standard - 1
                } else {
                    0
                };
                // The first upgrade slot follows the tier's storage.
                let slot = [27, 45, 63, 81, 99].get(tier).copied().unwrap_or(27);
                let tier_part = if tier > 0 { format!(",\"travelersbackpack:tier\":{tier}") } else { String::new() };
                // Its tanks on its model (`RENDER_INFO`), its bag red.
                *snbt = format!(
                    "{{count:1,id:\"{id}\",components:{{\"minecraft:container\":[{{slot:{slot},item:{{id:\"travelersbackpack:tanks_upgrade\",count:1}}}}],\"minecraft:custom_model_data\":{{flags:[1b],strings:[\"red\"]}}{tier_part}}}}}"
                );
            }
        }
    }
    core::ops::ControlFlow::Break(result)
}
