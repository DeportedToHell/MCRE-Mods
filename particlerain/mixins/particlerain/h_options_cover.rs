// The options stay hidden under the config screen.
fn particlerain_options_cover(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::particlerain_screen::covers(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
