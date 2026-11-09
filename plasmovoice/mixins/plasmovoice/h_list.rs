// The mod's row in Options > Mods.
fn plasmovoice_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::plasmovoice_glue::list(mods);
}
