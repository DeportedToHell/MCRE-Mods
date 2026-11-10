// The mod's row in Options > Mods.
fn particlerain_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::particlerain_glue::list(mods);
}
