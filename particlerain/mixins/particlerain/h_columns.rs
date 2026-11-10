// The game's rain and snow columns: kept only as the settings ask (the game's own geometry,
// made once more inside, then filtered).
fn particlerain_columns(
    game: &mut crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
) -> core::ops::ControlFlow<Vec<horizon_client::weather::Vertex>> {
    crate::game::particlerain_glue::columns(game, camera, alpha)
}
