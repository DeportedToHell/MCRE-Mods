// `LootHandler`: backpacks and tier upgrades in the chests of structures, as
// the original adds them (`enableLoot` in config/travelersbackpack-common.toml,
// on unless set to false).
fn travelersbackpack_loot_table_load(id: &str, table: &mut serde_json::Value) {
    use serde_json::json;
    let pools: &[(&str, f64)] = match id {
        "minecraft:chests/abandoned_mineshaft" => &[("bat", 0.05), ("#dyed", 0.06), ("iron_tier_upgrade", 0.05), ("gold_tier_upgrade", 0.04)],
        "minecraft:chests/pillager_outpost" => &[("iron_golem", 0.06)],
        "minecraft:chests/simple_dungeon" => &[("#dyed", 0.06), ("iron_tier_upgrade", 0.05)],
        "minecraft:chests/desert_pyramid" => &[("#dyed", 0.06), ("iron_tier_upgrade", 0.05), ("gold_tier_upgrade", 0.04)],
        "minecraft:chests/shipwreck_treasure" | "minecraft:chests/woodland_mansion" => {
            &[("iron_tier_upgrade", 0.06), ("gold_tier_upgrade", 0.05)]
        }
        "minecraft:chests/nether_bridge" | "minecraft:chests/bastion_treasure" => &[("iron_tier_upgrade", 0.07), ("gold_tier_upgrade", 0.06)],
        "minecraft:chests/end_city_treasure" => &[("gold_tier_upgrade", 0.07), ("diamond_tier_upgrade", 0.06)],
        _ => return,
    };
    // The config the game reads from its `config/` folder (the game's own folder is the working one).
    let config = std::env::current_dir().map(|d| d.join("config").join("travelersbackpack-common.toml"));
    let off = config.ok().and_then(|p| std::fs::read_to_string(p).ok()).is_some_and(|text| {
        text.lines().any(|l| {
            let l = l.trim();
            l.starts_with("enableLoot") && l.split('=').nth(1).is_some_and(|v| v.trim() == "false")
        })
    });
    if off {
        return;
    }
    // `DyeColor.getTextureDiffuseColor`, in `DyeColor` order.
    const DYES: [i64; 16] = [
        0xF9FFFE, 0xF9801D, 0xC74EBD, 0x3AB3DA, 0xFED83D, 0x80C71F, 0xF38BAA, 0x474F52, 0x9D9D97, 0x169C9C, 0x8932B8, 0x3C44AA,
        0x835432, 0x5E7C16, 0xB02E26, 0x1D1D21,
    ];
    let Some(list) = table.as_object_mut().map(|t| t.entry("pools").or_insert_with(|| json!([]))).and_then(|p| p.as_array_mut()) else {
        return;
    };
    for &(item, chance) in pools {
        let entries = if item == "#dyed" {
            // `addRandomDyedBackpackLoot`: the standard backpack, two times in three dyed.
            let mut entries = vec![json!({"type": "minecraft:item", "name": "travelersbackpack:standard", "weight": 272})];
            for colour in DYES {
                entries.push(json!({
                    "type": "minecraft:item",
                    "name": "travelersbackpack:standard",
                    "weight": 33,
                    "modifier": {"type": "minecraft:set_components", "components": {"minecraft:dyed_color": colour}}
                }));
            }
            entries
        } else {
            vec![json!({"type": "minecraft:item", "name": format!("travelersbackpack:{item}")})]
        };
        list.push(json!({
            "rolls": 1,
            "entries": entries,
            "condition": {"type": "minecraft:random_chance", "chance": chance}
        }));
    }
}
