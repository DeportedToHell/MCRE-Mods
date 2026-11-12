// Traveler's Backpack's row in Options > Mods, with its keys.
fn travelersbackpack_mods(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::travelersbackpack_glue::mods(mods);
}
