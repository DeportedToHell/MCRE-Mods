// `RightClickHandler`'s quick unequip (`rightClickUnequip`): sneaking with an
// empty main hand, a right click on a block puts the worn backpack there.
fn travelersbackpack_right_click_v2(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    face: [i32; 3],
    hand: horizon_client::first_person::Hand,
    cancel: &mut bool,
) {
    let main = matches!(hand, horizon_client::first_person::Hand::Main);
    crate::game::travelersbackpack_glue::quick_unequip(game, pos, face, main, cancel);
}
