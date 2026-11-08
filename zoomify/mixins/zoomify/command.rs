// `/zoomify` opens the settings.
fn zoomify_command(command: &str, ran: &mut bool) {
    crate::game::zoomify_glue::command(command, ran);
}
