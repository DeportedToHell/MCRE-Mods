// A player stops tracking another.
fn emotecraft_stop_tracking(game: &mut crate::game::Game<'_>, player: u128, uuid: u128, kind: &str) {
    crate::game::emotecraft_server::tracking(game, player, uuid, kind, false);
}
