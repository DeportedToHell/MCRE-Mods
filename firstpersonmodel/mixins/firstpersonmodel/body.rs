// In first person: the own body drawn moved behind the camera, its head hidden.
fn firstpersonmodel_body(
    game: &crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) -> core::ops::ControlFlow<()> {
    if crate::game::firstpersonmodel_glue::player_geometry(game, camera, alpha, out) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
