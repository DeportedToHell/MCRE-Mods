// A key went down or up: the toggle key.
fn firstpersonmodel_key(game: &crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::firstpersonmodel_glue::key(game, key, pressed);
}
