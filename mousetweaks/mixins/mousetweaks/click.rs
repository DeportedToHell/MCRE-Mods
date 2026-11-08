// A click on the settings page, or Mod Menu's Configure: the first button
// opens the options screen.
fn mousetweaks_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::mousetweaks_glue::page_click(game, mod_id, button);
}
