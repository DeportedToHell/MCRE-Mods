// /lambdynamiclights (or /lambdynlights): the settings page, and switching an entity type's light.
fn lambdynamiclights_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    crate::game::lambdynamiclights_glue::command(game, command, ran);
}
