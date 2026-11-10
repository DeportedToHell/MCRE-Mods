// The command's name, so the chat box completes and colours it.
fn particlerain_commands(commands: &mut Vec<&'static str>) {
    crate::game::particlerain_glue::commands(commands);
}
