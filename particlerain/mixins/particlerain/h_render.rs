// The particles' quads: Particle Rain's weather particles first.
fn particlerain_render(out: &mut Vec<crate::mesher::Vertex>) {
    crate::particles::particlerain_particle::append(out);
}
