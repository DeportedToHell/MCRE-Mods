// A backpack used on a block: it opens, or with sneak held it is placed.
fn travelersbackpack_use_item_on(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, pos: [i32; 3], claimed: &mut bool) {
    crate::game::travelersbackpack_glue::use_item_on(game, stack, pos, claimed);
}
