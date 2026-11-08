// Once a frame: the tables, and what the screen hooks asked for.
fn justenoughitems_frame(game: &mut crate::game::Game<'_>) {
    crate::game::justenoughitems_glue::frame(game);
}
