// A player left: their channel and rate limiter go.
fn pingwheel_logged_out(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::pingwheel_glue::logged_out(game, player);
}
