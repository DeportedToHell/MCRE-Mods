// `/tb_client`'s name, so the chat box completes and colours it.
fn travelersbackpack_client_commands(commands: &mut Vec<&'static str>) {
    crate::game::travelersbackpack_glue::client_commands(commands);
}
