// The mod's page in Options > Mods: Configure... first.
fn particlerain_settings(mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::particlerain_glue::page(mod_id, buttons);
}
