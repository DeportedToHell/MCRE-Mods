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
    // `upgradeInventory`: the storage grows; the upgrades and the tools keep
    // their own slots, after the bigger storage (storage, upgrades, tools a tier).
    const SLOTS: [(u16, u16, u16); 5] = [(27, 2, 2), (45, 3, 3), (63, 4, 4), (81, 5, 5), (99, 6, 6)];
    let (from, to) = (SLOTS[upgrade], SLOTS[upgrade + 1]);
    for (slot, _) in data.container.iter_mut() {
        let n = u16::from(*slot);
        let moved = if n < from.0 {
            n
        } else if n < from.0 + from.1 {
            to.0 + (n - from.0)
        } else if n < from.0 + from.1 + from.2 {
            to.0 + to.1 + (n - from.0 - from.1)
        } else {
            n
        };
        *slot = u8::try_from(moved).unwrap_or(*slot);
    }
    data.wire = Default::default();
    data.set_other(&format!("!{TIER}"), None);
    data.set_other(TIER, Some(horizon_world::nbt::Tag::Int(tier + 1)));
    result.tidy();
    Break(Some(result))
}
