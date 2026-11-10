// The world's primitives: the voice distance's sphere.
fn plasmovoice_primitives(game: &crate::game::Game<'_>, primitives: &mut horizon_client::world_primitives::WorldPrimitives) {
    crate::game::plasmovoice_glue::world_primitives(game, primitives);
}
