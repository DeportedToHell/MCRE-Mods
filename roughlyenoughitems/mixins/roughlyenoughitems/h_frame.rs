// Once a frame: the tables, and what the screen hooks asked for.
fn roughlyenoughitems_frame(game: &mut crate::game::Game<'_>) {
    crate::game::roughlyenoughitems_glue::frame(game);
}
