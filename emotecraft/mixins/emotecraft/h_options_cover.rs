// The options stay hidden under Emotecraft's screens.
fn emotecraft_options_cover(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::emotecraft_ui::options_covered(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
