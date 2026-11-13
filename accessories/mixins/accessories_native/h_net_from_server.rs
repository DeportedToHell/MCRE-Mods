// The server tells what each player wears.
fn accessories_net_from_server(channel: &str, data: &[u8]) {
    crate::game::accessories_glue::net_from_server(channel, data);
}
