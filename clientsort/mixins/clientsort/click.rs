// A click on the mod's settings page.
fn clientsort_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::clientsort_glue::page_click(game, mod_id, button);
}
