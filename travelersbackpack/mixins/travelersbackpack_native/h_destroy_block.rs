// A placed backpack being broken: its contents go to its drop.
fn travelersbackpack_destroy_block(game: &mut crate::game::Game<'_>, pos: [i32; 3]) {
    crate::game::travelersbackpack_glue::destroy_block(game, pos);
}
