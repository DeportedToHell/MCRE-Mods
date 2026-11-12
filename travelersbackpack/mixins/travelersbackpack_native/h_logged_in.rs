// A player joined: who wears what is told.
fn travelersbackpack_logged_in(player: u128) {
    crate::game::travelersbackpack_glue::logged_in(player);
}
