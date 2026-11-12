// The start of the game's tick: the keys a screen lets the player hold.
fn invmove_tick(game: &mut crate::game::Game<'_>) {
    crate::game::invmove_glue::input_tick(game);
}
