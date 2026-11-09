// A play packet from the server: Plasmo Voice's on `plasmo:voice/v2`.
fn plasmovoice_packet(game: &mut crate::game::Game<'_>, packet: &horizon_client::mod_api::ClientboundPlayPacket) {
    crate::game::plasmovoice_glue::packet(game, packet);
}
