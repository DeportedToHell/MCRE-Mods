// The blaze backpack hears projectile hits.
fn travelersbackpack_hears_impacts(hears: &mut bool) {
    crate::game::travelersbackpack_glue::hears_projectile_impacts(hears);
}
