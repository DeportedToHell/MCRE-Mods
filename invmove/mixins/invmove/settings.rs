// The mod's page in Options > Mods: Configure... first.
fn invmove_settings(mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::invmove_glue::page(mod_id, buttons);
}
