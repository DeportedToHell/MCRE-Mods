// A message from a player (on the host): the handshake, an ender chest request.
fn shulkerboxtooltip_from_client(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::shulkerboxtooltip_glue::from_client(game, player, channel, data);
}
