// The jump key on a screen whose focused button would take it: kept for
// jumping where the player can move.
fn invmove_jump_key(game: &mut crate::game::Game<'_>, key: winit::keyboard::KeyCode) -> core::ops::ControlFlow<bool> {
    if crate::game::invmove_glue::jump_key_kept(game, key) {
        return core::ops::ControlFlow::Break(false);
    }
    core::ops::ControlFlow::Continue(())
}
