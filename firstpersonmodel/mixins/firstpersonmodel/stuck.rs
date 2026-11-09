// ArrowLayer and BeeStingerLayer on the own body in first person: "Render
// Arrows & Stings", and each hidden on its own in "Show Layers".
fn firstpersonmodel_stuck(
    game: &crate::game::Game<'_>,
    m: &Mob,
    id: i32,
    camera: glam::DVec3,
    partial: f32,
    light: u32,
    out: &mut Vec<horizon_client::mesher::Vertex>,
) -> core::ops::ControlFlow<()> {
    if crate::game::firstpersonmodel_glue::stuck_in_body(game, m, id, camera, partial, light, out) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
