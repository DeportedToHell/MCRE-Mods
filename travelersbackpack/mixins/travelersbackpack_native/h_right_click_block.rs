// A placed backpack opens on a right click.
fn travelersbackpack_right_click_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::travelersbackpack_glue::right_click_block(game, pos, canceled);
}
