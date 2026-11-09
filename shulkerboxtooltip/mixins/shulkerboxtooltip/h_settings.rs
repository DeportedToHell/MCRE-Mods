// The mod's page (Options > Mods > Shulker Box Tooltip): Configure...
fn shulkerboxtooltip_settings(mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::shulkerboxtooltip_glue::page(mod_id, buttons);
}
