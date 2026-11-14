// The grave compass opens its grave.
fn universalgraves_use_item(game: &mut crate::game::Game<'_>, stack: &horizon_client::items::ItemStack) {
    crate::game::universalgraves::use_item(game, stack);
}
