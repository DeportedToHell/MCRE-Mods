// A player starts tracking another: their running emote comes to them mid-way.
fn emotecraft_start_tracking(game: &mut crate::game::Game<'_>, player: u128, uuid: u128, kind: &str) {
    crate::game::emotecraft_server::tracking(game, player, uuid, kind, true);
}
