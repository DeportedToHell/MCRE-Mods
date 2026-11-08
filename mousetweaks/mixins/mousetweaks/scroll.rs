// The wheel over a container screen, where Horizon's own inventory tweaks
// would take it: Mouse Tweaks' wheel instead while it is on.
fn mousetweaks_scroll(
    game: &mut crate::game::Game<'_>,
    delta: f32,
    slot: Option<usize>,
) -> core::ops::ControlFlow<bool> {
    match crate::game::mousetweaks_glue::scroll(game, delta, slot) {
        Some(took) => core::ops::ControlFlow::Break(took),
        None => core::ops::ControlFlow::Continue(()),
    }
}
