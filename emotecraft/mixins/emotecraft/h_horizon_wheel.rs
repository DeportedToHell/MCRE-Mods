// Horizon's own emote wheel: while Emotecraft is on, its key (B) opens Emotecraft's wheel.
fn emotecraft_horizon_wheel(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::emotecraft_glue::horizon_wheel_blocked(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
