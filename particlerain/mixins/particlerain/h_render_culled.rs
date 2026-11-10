// The particles' quads with culling (without it, `render` adds them).
fn particlerain_render_culled(culling_enabled: bool, out: &mut Vec<crate::mesher::Vertex>) {
    if culling_enabled {
        crate::particles::particlerain_particle::append(out);
    }
}
