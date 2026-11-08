// Options > Mods: the mod's row and keys.
fn roughlyenoughitems_list(
    game: &mut crate::game::Game<'_>,
    mods: &mut Vec<crate::game::native_hooks::NativeMod>,
) {
    crate::game::roughlyenoughitems_glue::mods(game, mods);
}
