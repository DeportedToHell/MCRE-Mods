// A click on the mod's settings page.
fn smoothswapping_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::smoothswapping_glue::page_click(game, mod_id, button);
}
