// The command's name, so the chat box completes and colours it.
fn pickupnotifier_commands(commands: &mut Vec<&'static str>) {
    crate::game::pickupnotifier_glue::commands(commands);
}
