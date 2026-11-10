// A player joined: Emotecraft's channels registered with them.
fn emotecraft_logged_in(game: &mut crate::game::Game<'_>, player: u128, name: &str) {
    crate::game::emotecraft_server::logged_in(game, player, name);
}
