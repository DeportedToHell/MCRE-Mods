// The command's names, so the chat box completes and colours it.
fn lambdynamiclights_commands(commands: &mut Vec<&'static str>) {
    crate::game::lambdynamiclights_glue::commands(commands);
}
