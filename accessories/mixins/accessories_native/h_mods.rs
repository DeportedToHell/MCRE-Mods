// Accessories' row in Options > Mods, with its key.
fn accessories_mods(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::accessories_glue::mods(mods);
}
