// The mod's row in Options > Mods.
fn shulkerboxtooltip_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::shulkerboxtooltip_glue::list(mods);
}
