// The frame's entities and particles: Particle Rain's quads for the particle pass.
fn particlerain_geometry(game: &crate::game::Game<'_>, camera: glam::DVec3, alpha: f64, fov_degrees: f32) {
    crate::game::particlerain_weather::frame(game, camera, alpha as f32, fov_degrees);
}
