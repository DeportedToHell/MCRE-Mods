// Typing over the title screen goes to the mods screen's search box.
fn modmenu_title_edit(
    game: &mut crate::game::Game<'_>,
    typed: Option<&str>,
    backspace: bool,
) -> core::ops::ControlFlow<()> {
    if crate::game::modmenu_glue::title_typed(game, typed, backspace) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
