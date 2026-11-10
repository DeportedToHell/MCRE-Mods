// The elytra while an avatar emotes: drawn following its body, elytra and cape bones.
fn emotecraft_elytra(game: &crate::game::Game<'_>, m: &horizon_client::mobs::Mob, chest: Option<&str>, camera: glam::DVec3, partial: f32, light: u32, out: &mut Vec<horizon_client::mesher::Vertex>) -> core::ops::ControlFlow<()> {
    if crate::game::emotecraft_glue::elytra(game, m, chest, camera, partial, light, out) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
