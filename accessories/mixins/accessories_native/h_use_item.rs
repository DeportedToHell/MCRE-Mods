// An item used in the air may go on as an accessory.
fn accessories_use_item(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack, claimed: &mut bool) {
    crate::game::accessories_glue::use_item(game, stack, claimed);
}
