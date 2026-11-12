// A wearer touches a stack: the pickup upgrade takes it first.
fn travelersbackpack_item_picked_up(
    game: &crate::game::Game<'_>,
    player: u128,
    stack: &horizon_client::items::ItemStack,
    position: [f64; 3],
    canceled: &mut bool,
) {
    crate::game::travelersbackpack_glue::item_picked_up(game, player, stack, position, canceled);
}
