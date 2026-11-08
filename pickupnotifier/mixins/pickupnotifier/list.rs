// The mod's row in Options > Mods.
fn pickupnotifier_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::pickupnotifier_glue::listed(mods);
}
