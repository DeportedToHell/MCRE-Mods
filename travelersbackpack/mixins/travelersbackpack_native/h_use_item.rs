// A backpack in the main hand opens on a right click.
fn travelersbackpack_use_item(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    crate::game::travelersbackpack_glue::use_item(game, stack, claimed);
}
