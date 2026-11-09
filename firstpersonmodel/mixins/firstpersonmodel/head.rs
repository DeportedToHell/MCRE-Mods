// CustomHeadLayer (never) on the own body in first person.
fn firstpersonmodel_head(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::firstpersonmodel_glue::skip_layer(game, crate::game::firstpersonmodel_glue::Layer::CustomHead) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
