// The backpack open from the hand can't be thrown (`BackpackBaseMenu`'s locked
// slot): its menu holds what it holds.
fn travelersbackpack_drop_held(game: &mut crate::game::Game<'_>) -> core::ops::ControlFlow<()> {
    if crate::game::travelersbackpack_glue::held_locked(game) {
        core::ops::ControlFlow::Break(())
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
