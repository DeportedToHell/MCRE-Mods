// Configure... on the mod's page, or Mod Menu's Configure: the Emotes config screen.
fn emotecraft_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::emotecraft_glue::page_click(game, mod_id, button);
}
