// The mod's row in Options > Mods, with its toggle key.
fn dynamiclights_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::dynamiclights_glue::list(mods);
}
