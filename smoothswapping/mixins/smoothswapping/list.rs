// The mod's row in Options > Mods.
fn smoothswapping_list(game: &mut crate::game::Game<'_>, mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::smoothswapping_glue::list(game, mods);
}
