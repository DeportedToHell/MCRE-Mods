// A message from a client: open the backpack, the hose, the keys, the trash bin's slots.
fn travelersbackpack_net_from_client(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::travelersbackpack_glue::net_from_client(game, player, channel, data);
}
