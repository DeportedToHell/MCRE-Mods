// The open accessories screen laid out again (scrolled, filtered, switched to
// the cosmetics): the slots Horizon places come from the mod's layout anew.
// Ported from Accessories by Wisp Forest (MIT).
pub(crate) fn relayout(game: &mut crate::game::Game<'_>) {
    let Some(open) = game.mod_screen.as_mut() else {
        return;
    };
    let mut screen = None;
    crate::game::accessories_glue::screen(&open.block, &mut screen);
    if let Some(screen) = screen {
        open.screen = screen;
    }
}

/// The menu type of the mod screen shown, if one is.
pub(crate) fn menu(game: &crate::game::Game<'_>) -> Option<String> {
    game.mod_screen.as_ref().map(|o| o.block.clone())
}
