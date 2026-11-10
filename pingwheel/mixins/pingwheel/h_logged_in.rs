// A player joined: the channels the server takes, registered with their client.
fn pingwheel_logged_in(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::pingwheel_glue::logged_in(game, player);
}
