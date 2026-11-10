// The mod's row in Options > Mods, with its keys.
fn pingwheel_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::pingwheel_glue::list(mods);
}
