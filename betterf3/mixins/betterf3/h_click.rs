// Configure... on the mod's page, or Mod Menu's Configure.
fn betterf3_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::betterf3_screens::page_click(game, mod_id, button);
}
