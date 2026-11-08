// The Options screen stays hidden under the config screen.
fn pickupnotifier_options_cover(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::pickupnotifier_config::covers(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
