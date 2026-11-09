// A click on the mod's page, or Mod Menu's Configure: Plasmo Voice's Mod Menu screen.
fn plasmovoice_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::plasmovoice_glue::page_click(game, mod_id, button);
}
