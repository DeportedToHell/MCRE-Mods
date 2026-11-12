// A blaze's small fireball does nothing to the blaze backpack's wearer.
fn travelersbackpack_projectile_impact(game: &mut crate::game::Game<'_>, kind: &str, entity: Option<u32>, entity_kind: &str, at: [f64; 3], canceled: &mut bool) {
    crate::game::travelersbackpack_glue::projectile_impact(game, kind, entity, entity_kind, at, canceled);
}
