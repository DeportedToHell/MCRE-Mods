// `backSlotIntegration` (`BackpackTrinket`, `TravelersBackpackAccessory`): with
// Accessories, a backpack goes into its Back slot (`canEquip`), never from a
// right click (`canEquipFromUse`), and is drawn by this mod's own layer, not
// Accessories' default renderer. As item defaults, which Accessories reads.
fn travelersbackpack_default_components(item: &str, components: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(name) = item.strip_prefix("travelersbackpack:") else {
        return;
    };
    let backpack = !name.ends_with("_upgrade")
        && !name.ends_with("_sleeping_bag")
        && name != "backpack_tank"
        && name != "hose"
        && name != "hose_nozzle";
    if !backpack {
        return;
    }
    // The server config's `backSlotIntegration` (default true), read once.
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let on = *ON.get_or_init(|| {
        let text = std::env::current_dir()
            .ok()
            .and_then(|d| std::fs::read_to_string(d.join("config").join("travelersbackpack-server.toml")).ok())
            .unwrap_or_default();
        !text.lines().map(str::trim).any(|l| {
            l.starts_with("backSlotIntegration") && l.split_once('=').is_some_and(|(_, v)| v.trim() == "false")
        })
    });
    if !on {
        return;
    }
    components.insert("accessories:slot_validation".into(), serde_json::json!({ "valid_slots": ["back"] }));
    components.insert("accessories:stack_settings".into(), serde_json::json!({ "can_equip_from_use": false }));
    components.insert("accessories:custom_renderer".into(), serde_json::json!({ "rendering_functions": [] }));
}
