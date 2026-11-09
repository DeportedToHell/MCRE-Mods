// ParrotOnShoulderLayer on the own body in first person.
fn firstpersonmodel_parrots(game: &crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::firstpersonmodel_glue::skip_layer(game, crate::game::firstpersonmodel_glue::Layer::Parrots) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
