// The mod's row in Options > Mods.
fn bwncr_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::bwncr_glue::list(mods);
}