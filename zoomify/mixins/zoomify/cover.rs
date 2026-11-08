// The Options screen stays hidden under Zoomify's config screen.
fn zoomify_cover(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::zoomify_screen::covers(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
