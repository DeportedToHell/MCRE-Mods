// The lapis backpack hears experience orbs picked up.
fn travelersbackpack_xp_listened(listened: &mut bool) {
    crate::game::travelersbackpack_glue::experience_orb_pickup_listened(listened);
}
