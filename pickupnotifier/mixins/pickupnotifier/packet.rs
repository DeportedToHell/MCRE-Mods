// A play packet from the server, before the game handles it: pickups
// (`ClientboundTakeItemEntity`) join the list; a new player empties it.
fn pickupnotifier_packet(
    game: &mut crate::game::Game<'_>,
    packet: &horizon_client::mod_api::ClientboundPlayPacket,
) {
    crate::game::pickupnotifier_glue::packet(game, packet);
}
