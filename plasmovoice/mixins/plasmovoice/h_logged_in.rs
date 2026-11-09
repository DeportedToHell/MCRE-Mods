// A player joined: the voice server asks their client about it.
fn plasmovoice_logged_in(game: &mut crate::game::Game<'_>, player: u128, name: &str) {
    crate::game::plasmovoice_glue::logged_in(game, player, name);
}
