// The mod's row in Options > Mods.
fn fallingleaves_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::fallingleaves_glue::list(mods);
}
