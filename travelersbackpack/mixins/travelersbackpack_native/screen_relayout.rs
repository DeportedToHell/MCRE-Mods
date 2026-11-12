// The open backpack screen laid out again (its storage scrolled): the slots
// Horizon places come from the mod's layout anew.
pub(crate) fn relayout(game: &mut crate::game::Game<'_>) {
    let Some(open) = game.mod_screen.as_mut() else {
        return;
    };
    let mut screen = None;
    crate::game::travelersbackpack_glue::screen(&open.block, &mut screen);
    if let Some(screen) = screen {
        open.screen = screen;
    }
}
