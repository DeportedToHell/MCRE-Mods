// Each frame, in place of Horizon's own gathering: this mod's light sources
// for the dynamic light shader. Continue: the mod is off, Horizon gathers.
fn lambdynamiclights_gather(
    game: &mut crate::game::Game<'_>,
    camera: glam::DVec3,
    alpha: f64,
) -> core::ops::ControlFlow<()> {
    crate::game::lambdynamiclights_glue::gather(game, camera, alpha)
}
