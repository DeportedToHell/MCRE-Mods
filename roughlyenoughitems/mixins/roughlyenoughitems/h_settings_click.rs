// A click on the settings page.
fn roughlyenoughitems_settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::roughlyenoughitems_glue::settings_click(game, mod_id, button);
}
