// The mod's row in Options > Mods.
fn mousetweaks_list(game: &mut crate::game::Game<'_>, mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::mousetweaks_glue::list(game, mods);
}
