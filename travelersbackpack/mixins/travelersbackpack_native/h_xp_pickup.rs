// The lapis backpack doubles an orb now and then.
fn travelersbackpack_xp_pickup(game: &mut crate::game::Game<'_>, player: u128, value: &mut u32) {
    crate::game::travelersbackpack_glue::experience_orb_pickup(game, player, value);
}
