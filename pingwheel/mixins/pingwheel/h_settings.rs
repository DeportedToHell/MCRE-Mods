// The mod's page in Options > Mods: Configure... first.
fn pingwheel_settings(mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::pingwheel_glue::page(mod_id, buttons);
}
