// The mod's row in Options > Mods, with its keys.
fn zoomify_list(game: &mut crate::game::Game<'_>, mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::zoomify_glue::list(game, mods);
}
