// The bee, wither and warden backpacks' hits.
fn travelersbackpack_hurt_enemy(game: &mut crate::game::Game<'_>, entity: u32, kind: &str) {
    crate::game::travelersbackpack_glue::hurt_enemy(game, entity, kind);
}
