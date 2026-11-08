// The options stay hidden under ClientSort's config screen.
fn clientsort_options_cover(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::clientsort_config::covers(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
