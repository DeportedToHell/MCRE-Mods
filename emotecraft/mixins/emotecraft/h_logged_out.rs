// A player left.
fn emotecraft_logged_out(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::emotecraft_server::logged_out(game, player);
}
