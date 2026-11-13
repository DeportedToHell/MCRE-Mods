// `BackpackUpgradeRecipe.assemble`: leather, a backpack and the upgrade to its
// next tier make that backpack one tier up, holding what it held; any other
// tier upgrade makes nothing. (Its own copy of the tiers' upgrades: this runs
// in the game's library, away from the mod's other modules.)
fn travelersbackpack_smithing(
    template: Option<&crate::items::ItemStack>,
    base: Option<&crate::items::ItemStack>,
    addition: Option<&crate::items::ItemStack>,
) -> core::ops::ControlFlow<Option<ItemStack>> {
    use core::ops::ControlFlow::{Break, Continue};
    // `Tiers.getTierUpgradeIngredient`, leather's first.
    const UPGRADES: [&str; 4] = [
        "travelersbackpack:iron_tier_upgrade",
        "travelersbackpack:gold_tier_upgrade",
        "travelersbackpack:diamond_tier_upgrade",
        "travelersbackpack:netherite_tier_upgrade",
    ];
    const TIER: &str = "travelersbackpack:tier";
    let (Some(template), Some(base), Some(addition)) = (template, base, addition) else {
        return Continue(());
    };
    let Some(upgrade) = UPGRADES.iter().position(|u| *u == &*addition.item) else {
        return Continue(());
    };
    // Only a backpack (`TravelersBackpackItem`): not the hose, a tank, a bag or an upgrade.
    let Some(name) = base.item.strip_prefix("travelersbackpack:") else {
        return Continue(());
    };
    let not_backpack = name.ends_with("_upgrade") || name.ends_with("_sleeping_bag") || ["hose", "hose_nozzle", "backpack_tank"].contains(&name);
    if &*template.item != "minecraft:leather" || not_backpack {
        return Continue(());
    }
    let tier = match base.data().and_then(|d| d.other(TIER)) {
        Some(horizon_world::nbt::Tag::Int(n)) => *n,
        _ => 0,
    };
    if usize::try_from(tier).ok() != Some(upgrade) {
        return Break(None);
    }
    let mut result = base.clone();
    result.count = 1;
    let data = result.data_mut();
    // `upgradeInventory`: the next tier's sizes from the config (`STORAGE_SLOTS`
    // …); the storage grows, the upgrades and the tools keep their own slots
    // after it. The backpack's own sizes are those it was made with: without
    // them, its tier's from the config when empty, else the first ones.
    const SLOTS: [(u16, u16, u16); 5] = [(27, 2, 2), (45, 3, 3), (63, 4, 4), (81, 5, 5), (99, 6, 6)];
    const SIZES: [&str; 3] = ["travelersbackpack:storage_slots", "travelersbackpack:upgrade_slots", "travelersbackpack:tool_slots"];
    let config = |n: usize| {
        let [s, u, t] = crate::lan::containers::travelersbackpack_menu::tier_sizes(n);
        (s as u16, u as u16, t as u16)
    };
    let own: Vec<Option<u16>> = SIZES
        .iter()
        .map(|id| match data.other(id) {
            Some(horizon_world::nbt::Tag::Int(n)) => u16::try_from(*n).ok(),
            _ => None,
        })
        .collect();
    let from = match own[..] {
        [Some(s), Some(u), Some(t)] => (s, u, t),
        _ if data.container.is_empty() => config(upgrade),
        _ => SLOTS[upgrade],
    };
    let to = config(upgrade + 1);
    let mut placed: Vec<(u16, ItemStack)> = Vec::new();
    let mut over = Vec::new();
    for (slot, item) in std::mem::take(&mut data.container) {
        let n = u16::from(slot);
        let moved = if n < from.0 {
            (n < to.0).then_some(n)
        } else if n < from.0 + from.1 {
            (n - from.0 < to.1).then(|| to.0 + (n - from.0))
        } else if n < from.0 + from.1 + from.2 {
            (n - from.0 - from.1 < to.2).then(|| to.0 + to.1 + (n - from.0 - from.1))
        } else {
            None
        };
        match moved {
            Some(m) => placed.push((m, item)),
            None => over.push(item),
        }
    }
    // What has no slot of its own in the new sizes (a config with fewer) goes
    // to a free storage slot, else after the tools.
    let mut spare = to.0 + to.1 + to.2;
    for item in over {
        let free = (0..to.0).find(|k| !placed.iter().any(|(m, _)| m == k));
        let at = free.unwrap_or_else(|| {
            spare += 1;
            spare - 1
        });
        placed.push((at, item));
    }
    data.container = placed.into_iter().filter_map(|(m, item)| Some((u8::try_from(m).ok()?, item))).collect();
    for (id, n) in SIZES.iter().zip([to.0, to.1, to.2]) {
        data.set_other(&format!("!{id}"), None);
        data.set_other(id, Some(horizon_world::nbt::Tag::Int(i32::from(n))));
    }
    data.wire = Default::default();
    data.set_other(&format!("!{TIER}"), None);
    data.set_other(TIER, Some(horizon_world::nbt::Tag::Int(tier + 1)));
    result.tidy();
    Break(Some(result))
}
