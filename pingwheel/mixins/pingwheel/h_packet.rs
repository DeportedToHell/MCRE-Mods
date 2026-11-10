// A play packet from the server: Ping Wheel's `ping-wheel-s2c:ping-location`.
fn pingwheel_packet(game: &mut crate::game::Game<'_>, packet: &horizon_client::mod_api::ClientboundPlayPacket) {
    crate::game::pingwheel_glue::packet(game, packet);
}
