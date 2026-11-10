// /particlerain, /particlerain debug, /particlerain biometags.
fn particlerain_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::particlerain_glue::command(game, command, ran);
}
