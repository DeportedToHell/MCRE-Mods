// The mod's row in Options > Mods, with its toggle key.
fn invmove_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::invmove_glue::list(mods);
}
