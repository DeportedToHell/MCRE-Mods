// `/zoomify` is the mod's.
fn zoomify_commands(commands: &mut Vec<&'static str>) {
    crate::game::zoomify_glue::commands(commands);
}
