// `data/travelersbackpack/villager_trade/wandering_trader/emerald_villager.json`:
// the wandering trader sells the villager backpack for 50 emeralds, once, as
// one of its uncommon trades (`#minecraft:wandering_trader/uncommon`). It joins
// the game's own trade files and tags as they are read.
fn travelersbackpack_trades(dir: &Path, out: &mut HashMap<String, Value>) -> core::ops::ControlFlow<()> {
    thread_local! {
        static INSIDE: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
    }
    if INSIDE.with(core::cell::Cell::get) {
        return core::ops::ControlFlow::Continue(());
    }
    INSIDE.with(|c| c.set(true));
    walk(dir, out);
    INSIDE.with(|c| c.set(false));
    // `enableVillagerTrade` in config/travelersbackpack-common.toml, on unless set to false.
    let off = std::env::current_dir()
        .ok()
        .and_then(|d| std::fs::read_to_string(d.join("config").join("travelersbackpack-common.toml")).ok())
        .is_some_and(|text| {
            text.lines().any(|l| {
                let l = l.trim();
                l.starts_with("enableVillagerTrade") && l.split('=').nth(1).is_some_and(|v| v.trim() == "false")
            })
        });
    if off {
        return core::ops::ControlFlow::Break(());
    }
    let id = "travelersbackpack:wandering_trader/emerald_villager";
    if dir.ends_with("tags/villager_trade") {
        if let Some(Value::Array(values)) = out
            .get_mut("minecraft:wandering_trader/uncommon")
            .and_then(|tag| tag.get_mut("values"))
            && !values.iter().any(|v| v.as_str() == Some(id))
        {
            values.push(Value::String(id.to_owned()));
        }
    } else if dir.ends_with("villager_trade") {
        out.insert(
            id.to_owned(),
            serde_json::json!({
                "gives": { "id": "travelersbackpack:villager" },
                "max_uses": 1,
                "reputation_discount": 0.05,
                "wants": { "count": 50, "id": "minecraft:emerald" },
                "xp": 50
            }),
        );
    }
    core::ops::ControlFlow::Break(())
}
