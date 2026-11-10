// The mod's page (Options > Mods > Dynamic Lights).
fn dynamiclights_settings(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::dynamiclights_glue::page(game, mod_id, buttons);
}
