// The server half: a player is about to take an item entity's stack; their
// client is told what it was, in case it never had the entity.
fn pickupnotifier_picked_up(
    game: &mut crate::game::Game<'_>,
    player: u128,
    stack: &horizon_client::items::ItemStack,
    position: [f64; 3],
) {
    crate::game::pickupnotifier_glue::item_picked_up(game, player, stack, position);
}
