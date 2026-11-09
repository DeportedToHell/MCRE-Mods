// The mod's settings page: the config screen's button.
fn firstpersonmodel_settings(mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::firstpersonmodel_glue::page(mod_id, buttons);
}
