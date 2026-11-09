// A player left.
fn plasmovoice_logged_out(game: &mut crate::game::Game<'_>, player: u128) {
    crate::game::plasmovoice_glue::logged_out(game, player);
}
