// The mod's page in Options > Mods: Configure... first.
fn bwncr_settings(mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::bwncr_glue::page(mod_id, buttons);
}