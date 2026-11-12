// Who wears which backpack, from the server.
fn travelersbackpack_net_from_server(channel: &str, data: &[u8]) {
    crate::game::travelersbackpack_glue::net_from_server(channel, data);
}
