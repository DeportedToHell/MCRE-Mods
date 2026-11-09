// The wheel while playing: a player's volume with the action key held on them.
fn plasmovoice_wheel(game: &mut crate::game::Game<'_>, steps: f32, claimed: &mut bool) {
    crate::game::plasmovoice_glue::wheel(game, steps, claimed);
}
