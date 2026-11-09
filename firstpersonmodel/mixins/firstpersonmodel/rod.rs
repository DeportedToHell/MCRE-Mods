// The fishing line leaves the body's hand while its arms show
// (FishingBobberRendererMixin): the third-person hand, moved with the body.
fn firstpersonmodel_rod(game: &crate::game::Game<'_>, alpha: f64) -> core::ops::ControlFlow<Option<glam::DVec3>> {
    match crate::game::firstpersonmodel_glue::fishing_hand(game, alpha) {
        Some(hand) => core::ops::ControlFlow::Break(hand),
        None => core::ops::ControlFlow::Continue(()),
    }
}
