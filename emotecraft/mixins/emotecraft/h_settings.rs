// The mod's page in Options > Mods: Configure... first.
fn emotecraft_settings(mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::emotecraft_glue::page(mod_id, buttons);
}
