// A placed backpack keeps the item's contents and tier.
fn travelersbackpack_place_block(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, pos: [i32; 3]) {
    crate::game::travelersbackpack_glue::place_block(game, stack, pos);
}
