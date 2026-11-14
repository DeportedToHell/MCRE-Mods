// A grave is broken by the mod, never by the game.
fn universalgraves_destroy_block(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    crate::game::universalgraves::destroy_block(game, pos, canceled);
}
