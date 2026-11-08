// The mod's settings page.
fn clientsort_settings(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::clientsort_glue::page(game, mod_id, buttons);
}
