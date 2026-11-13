// The slot holding an open backpack can't be clicked (`BackpackBaseMenu`'s
// locked slot): a click on it, or a number key swapping it, does nothing.
// A right click with a backpack on an item, or an item on a backpack, puts
// the item into it (`overrideStackedOnOther`, `overrideOtherStackedOnMe`).
fn travelersbackpack_lock_slot(
    game: &crate::game::Game<'_>,
    host: &mut crate::game::lan::LanHost,
    conn: u32,
    p: &mut horizon_protocol::packets::play::ContainerClick,
) {
    crate::game::travelersbackpack_glue::lock_slot(host, conn, p);
    crate::game::travelersbackpack_glue::stack_into_backpack(game, host, conn, p);
}
