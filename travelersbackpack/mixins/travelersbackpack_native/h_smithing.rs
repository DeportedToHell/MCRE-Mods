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
    if &*template.item != "minecraft:leather" || !base.item.starts_with("travelersbackpack:") {
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
    data.set_other(&format!("!{TIER}"), None);
    data.set_other(TIER, Some(horizon_world::nbt::Tag::Int(tier + 1)));
    result.tidy();
    Break(Some(result))
}
