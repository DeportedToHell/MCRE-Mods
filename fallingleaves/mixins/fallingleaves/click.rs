// A click on the mod's page, or Mod Menu's Configure: the settings screen.
fn fallingleaves_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::fallingleaves_glue::page_click(game, mod_id, button);
}
