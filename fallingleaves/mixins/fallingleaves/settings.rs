// The mod's page in Options > Mods: Configure... first.
fn fallingleaves_settings(mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::fallingleaves_glue::page(mod_id, buttons);
}
