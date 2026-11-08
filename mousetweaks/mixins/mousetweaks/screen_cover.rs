// The Options screen stays hidden under Mouse Tweaks' options screen.
fn mousetweaks_options_cover(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::mousetweaks_screen::covers(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
