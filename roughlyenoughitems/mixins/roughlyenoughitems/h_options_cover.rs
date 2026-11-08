// The Options screen stays hidden under REI's config screen.
fn roughlyenoughitems_options_cover(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::roughlyenoughitems_options::covers(game) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
