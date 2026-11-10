// Configure... on the mod's page, or Mod Menu's Configure: the settings screen.
fn pingwheel_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::pingwheel_glue::page_click(game, mod_id, button);
}
