// The Options screen stays hidden under Smooth Swapping's config screen.
fn smoothswapping_options_cover(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::smoothswapping_screen::covers(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
