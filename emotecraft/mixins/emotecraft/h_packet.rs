// A play packet from the server: Emotecraft's messages and the channels it registers.
fn emotecraft_packet(game: &mut crate::game::Game<'_>, packet: &horizon_client::mod_api::ClientboundPlayPacket) {
    crate::game::emotecraft_glue::packet(game, packet);
}
