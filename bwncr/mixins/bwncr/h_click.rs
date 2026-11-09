// Configure... on the mod's page, or Mod Menu's Configure.
fn bwncr_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::bwncr_glue::page_click(game, mod_id, button);
}