// The command's name, so the chat box completes and colours it.
fn bwncr_commands(commands: &mut Vec<&'static str>) {
    crate::game::bwncr_glue::commands(commands);
}