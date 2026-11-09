// The own body is in view while the mod draws it in first person.
fn firstpersonmodel_in_view(value: bool, game: &crate::game::Game<'_>) -> bool {
    crate::game::firstpersonmodel_glue::own_body_in_view(game, value)
}
