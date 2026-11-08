// Once a frame: the tables, what the screen hooks asked for, and the config
// screen over the Options screen.
fn roughlyenoughitems_frame(game: &mut crate::game::Game<'_>) {
    crate::game::roughlyenoughitems_glue::frame(game);
    crate::game::roughlyenoughitems_options::frame(game);
}
