// The mod's settings page.
fn smoothswapping_settings(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::smoothswapping_glue::page(game, mod_id, buttons);
}
