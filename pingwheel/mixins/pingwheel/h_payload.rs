// A guest's plugin message: a ping, or a channel.
fn pingwheel_payload(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::pingwheel_glue::payload(game, player, channel, data);
}
