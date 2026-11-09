// The mod's page in Options > Mods: Configure... first.
fn betterf3_settings(mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::betterf3_screens::page(mod_id, buttons);
}
