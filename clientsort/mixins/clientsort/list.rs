// The mod's row in Options > Mods, with its four keys.
fn clientsort_list(game: &mut crate::game::Game<'_>, mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::clientsort_glue::list(game, mods);
}
