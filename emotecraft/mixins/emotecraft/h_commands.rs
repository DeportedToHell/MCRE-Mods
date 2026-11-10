// The client's own commands: /emotes-client.
fn emotecraft_commands(game: &mut crate::game::Game<'_>, commands: &mut Vec<&'static str>) {
    crate::game::emotecraft_glue::commands(game, commands);
}
