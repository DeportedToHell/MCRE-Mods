// The mod's row in Options > Mods.
fn clumps_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::clumps_glue::list(mods);
}