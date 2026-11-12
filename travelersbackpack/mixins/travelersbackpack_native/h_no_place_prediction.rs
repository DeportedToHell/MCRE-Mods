// `TravelersBackpackItem.place`: in the main hand without sneaking a backpack
// is not placed (FAIL) but opened on the server, so this client predicts no
// block where it points (it showed there for a moment).
fn travelersbackpack_no_place_prediction(
    game: &crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
) -> core::ops::ControlFlow<Option<DryPlace>> {
    if crate::game::travelersbackpack::is_backpack(&stack.item) && !game.remote_shift_down() {
        core::ops::ControlFlow::Break(None)
    } else {
        core::ops::ControlFlow::Continue(())
    }
}
