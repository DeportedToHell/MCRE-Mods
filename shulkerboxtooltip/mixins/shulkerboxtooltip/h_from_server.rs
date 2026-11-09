// A message from the server: its configuration, the ender chest's contents.
fn shulkerboxtooltip_from_server(channel: &str, data: &[u8]) {
    crate::game::shulkerboxtooltip_glue::from_server(channel, data);
}
