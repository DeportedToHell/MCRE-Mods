// A click on the mod's page, or Mod Menu's Configure: the config screen.
fn particlerain_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::particlerain_screen::page_click(game, mod_id, button);
}
