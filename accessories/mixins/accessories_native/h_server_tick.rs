// Each level's tick (with its world and the hub): the players' accessories.
fn accessories_server_tick(game: &mut crate::game::Game<'_>) {
    crate::game::accessories_glue::server_tick(game);
}
