// "Always Both Hands": the empty off hand's arm shows too.
fn firstpersonmodel_both_hands(game: &crate::game::Game<'_>, partial: f32, out: &mut Vec<Vertex>) {
    crate::game::firstpersonmodel_glue::off_hand_arm(game, partial, out);
}
