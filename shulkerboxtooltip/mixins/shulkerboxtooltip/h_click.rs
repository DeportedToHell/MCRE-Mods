// A click on the page, or Mod Menu's Configure: the config screen opens.
fn shulkerboxtooltip_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::shulkerboxtooltip_glue::page_click(game, mod_id, button);
}
