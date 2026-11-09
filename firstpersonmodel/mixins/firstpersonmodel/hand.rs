// A vanilla first-person hand, hidden as the original's HeldItemRendererMixin hides it.
fn firstpersonmodel_hand(game: &crate::game::Game<'_>, canceled: &mut bool) {
    if !*canceled && crate::game::firstpersonmodel_glue::cancel_hand(game) {
        *canceled = true;
    }
}
