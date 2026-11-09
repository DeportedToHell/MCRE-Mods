// Each frame, before it is drawn.
fn firstpersonmodel_frame(game: &mut crate::game::Game<'_>) {
    crate::game::firstpersonmodel_glue::frame(game);
}
