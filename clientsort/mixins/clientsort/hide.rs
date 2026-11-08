// Horizon's own sort buttons give way to ClientSort's where those show: no
// "≡" by the titles, and no click or key on them.
fn clientsort_hide(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<Vec<((f32, f32), Vec<usize>)>> {
    if crate::game::clientsort_glue::hides_horizon_sort(game) {
        return core::ops::ControlFlow::Break(Vec::new());
    }
    core::ops::ControlFlow::Continue(())
}
