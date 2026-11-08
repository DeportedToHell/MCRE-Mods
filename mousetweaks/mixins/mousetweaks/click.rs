// A click on the settings page.
fn mousetweaks_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::mousetweaks_glue::page_click(game, mod_id, button);
}
