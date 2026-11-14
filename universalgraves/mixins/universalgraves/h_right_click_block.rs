// A right click on a grave opens it.
fn universalgraves_right_click_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::universalgraves::right_click_block(game, pos, canceled);
}
