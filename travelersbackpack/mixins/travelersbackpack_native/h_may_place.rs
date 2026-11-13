// `BackpackSlotItemHandler.isItemValid`: a backpack's own slots (its storage,
// a crafting grid, a furnace's input) take no backpack, no shulker box unless
// `allowShulkerBoxes`, nothing of `#travelersbackpack:blacklisted_items` or the
// config's `blacklistedItems`.
fn travelersbackpack_may_place(view: &View<'_>, i: usize, stack: &ItemStack, rules: &dyn Rules) -> core::ops::ControlFlow<bool> {
    // The server config's two settings, read once (`config/travelersbackpack-server.toml`).
    static CONFIG: std::sync::OnceLock<(bool, Vec<String>)> = std::sync::OnceLock::new();
    let (shulkers, blacklist) = CONFIG.get_or_init(|| {
        let text = std::env::current_dir()
            .ok()
            .and_then(|d| std::fs::read_to_string(d.join("config").join("travelersbackpack-server.toml")).ok())
            .unwrap_or_default();
        let value = |key: &str| {
            text.lines().map(str::trim).find(|l| l.starts_with(key) && l[key.len()..].trim_start().starts_with('=')).map(|l| l.split_once('=').map_or("", |(_, v)| v).trim().to_owned())
        };
        let shulkers = value("allowShulkerBoxes").as_deref() == Some("true");
        let blacklist = value("blacklistedItems")
            .map(|v| v.trim_matches(|c| c == '[' || c == ']').split(',').map(|s| s.trim().trim_matches('"').to_owned()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        (shulkers, blacklist)
    });
    let g = &*view.guest;
    let own = g.menu.as_ref().is_some_and(|m| crate::lan::containers::travelersbackpack_menu::is_open(g.conn, m.window) && i < m.size);
    if !own {
        return core::ops::ControlFlow::Continue(());
    }
    let item = &*stack.item;
    let backpack = item.strip_prefix("travelersbackpack:").is_some_and(|n| {
        !n.ends_with("_upgrade") && !n.ends_with("_sleeping_bag") && n != "backpack_tank" && n != "hose" && n != "hose_nozzle"
    });
    let shulker = item.ends_with("shulker_box") && !*shulkers;
    let banned = rules.items().item_in_tag("travelersbackpack:blacklisted_items", item) || blacklist.iter().any(|b| b == item);
    if backpack || shulker || banned {
        return core::ops::ControlFlow::Break(false);
    }
    core::ops::ControlFlow::Continue(())
}
