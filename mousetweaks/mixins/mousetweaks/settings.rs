// The mod's settings page (Options > Mods > Mouse Tweaks).
fn mousetweaks_settings(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::mousetweaks_glue::page(game, mod_id, buttons);
}
