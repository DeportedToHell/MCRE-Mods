// The Options screen stays hidden under the config screen.
fn firstpersonmodel_cover(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::firstpersonmodel_screen::covers(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
