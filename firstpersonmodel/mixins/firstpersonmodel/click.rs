// A click on the mod's settings page, or Mod Menu's Configure.
fn firstpersonmodel_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::firstpersonmodel_glue::page_click(game, mod_id, button);
}
