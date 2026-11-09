// A guest's plugin message: Plasmo Voice's on `plasmo:voice/v2`.
fn plasmovoice_payload(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::plasmovoice_glue::payload(game, player, channel, data);
}
