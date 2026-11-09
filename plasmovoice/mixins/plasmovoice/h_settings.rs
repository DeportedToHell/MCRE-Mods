// The mod's page in Options > Mods: Configure... first.
fn plasmovoice_settings(mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::plasmovoice_glue::page(mod_id, buttons);
}
