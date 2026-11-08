// The mod's settings page.
fn zoomify_settings(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::zoomify_glue::page(game, mod_id, buttons);
}
