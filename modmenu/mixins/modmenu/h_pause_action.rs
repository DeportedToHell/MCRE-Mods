// The pause menu's Mods button pressed.
fn modmenu_pause_action(game: &mut crate::game::Game<'_>, hit: u8) -> core::ops::ControlFlow<Result<()>> {
    if hit == crate::game::modmenu::PAUSE_BUTTON {
        crate::game::modmenu_glue::open_from_pause(game);
        return core::ops::ControlFlow::Break(Ok(()));
    }
    core::ops::ControlFlow::Continue(())
}
