// Digging a grave breaks it for one who may take from it.
fn universalgraves_left_click_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::universalgraves::left_click_block(game, pos, canceled);
}
