// A guest's plugin message: Emotecraft's on emotecraft:emote.
fn emotecraft_payload(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::emotecraft_server::payload(game, player, channel, data);
}
