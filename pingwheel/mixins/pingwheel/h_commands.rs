// The command's name, so the chat box completes and colours it.
fn pingwheel_commands(commands: &mut Vec<&'static str>) {
    crate::game::pingwheel_glue::commands(commands);
}
