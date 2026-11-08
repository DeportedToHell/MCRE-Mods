// Options > Mods: the mod's row and its key.
fn modmenu_list(game: &mut crate::game::Game<'_>, mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::modmenu_glue::mods(game, mods);
}
