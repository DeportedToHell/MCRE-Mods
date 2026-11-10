// A player's chat box asks the server to complete an argument: /vmute's duration, /vunmute's player.
fn plasmovoice_complete(
    game: &mut crate::game::Game<'_>,
    label: &str,
    arguments: &[String],
    range: [usize; 2],
    suggestions: &mut Vec<horizon_client::mod_api::Suggestion>,
) {
    crate::game::plasmovoice_glue::complete(game, label, arguments, range, suggestions);
}
