// The slot holding an open backpack can't be clicked (`BackpackBaseMenu`'s
// locked slot): a click on it, or a number key swapping it, does nothing.
fn travelersbackpack_lock_slot(
    host: &crate::game::lan::LanHost,
    conn: u32,
    p: &mut horizon_protocol::packets::play::ContainerClick,
) {
    crate::game::travelersbackpack_glue::lock_slot(host, conn, p);
}
