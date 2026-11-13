// `MobEntityMixin`: a mob that spawns may wear a backpack.
fn travelersbackpack_mob_spawned(game: &mut crate::game::Game<'_>, dimension: &str, kind: &str, pos: [f64; 3]) {
    crate::game::travelersbackpack_glue::mob_spawned(game, dimension, kind, pos);
}
