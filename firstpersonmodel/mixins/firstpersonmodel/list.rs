// The mod's row in Options > Mods, with its key.
fn firstpersonmodel_list(game: &mut crate::game::Game<'_>, mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::firstpersonmodel_glue::list(game, mods);
}
