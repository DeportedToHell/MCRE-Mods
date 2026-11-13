// The client's tick: what the screen asked.
fn accessories_client_tick(game: &mut crate::game::Game<'_>) {
    crate::game::accessories_glue::client_tick(game);
}
