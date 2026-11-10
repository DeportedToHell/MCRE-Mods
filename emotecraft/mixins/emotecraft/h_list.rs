// The mod's row in Options > Mods, with its keys.
fn emotecraft_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::emotecraft_glue::mods_list(mods);
}
