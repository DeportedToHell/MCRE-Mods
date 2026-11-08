// A click on the page, or Mod Menu's Configure: the first button opens the
// settings screen.
fn dynamiclights_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::dynamiclights_glue::page_click(game, mod_id, button);
}
