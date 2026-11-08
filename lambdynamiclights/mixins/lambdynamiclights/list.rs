// The mod's row in Options > Mods.
fn lambdynamiclights_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::lambdynamiclights_glue::listed(mods);
}
