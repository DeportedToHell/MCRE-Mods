// The backpack a broken one drops holds its contents and tier.
fn travelersbackpack_loot_drops(
    game: &mut crate::game::Game<'_>,
    context: &horizon_client::mod_api::NativeLootDrops<'_>,
    drops: &mut Vec<horizon_client::items::ItemStack>,
) {
    crate::game::travelersbackpack_glue::loot_drops(game, context, drops);
}
