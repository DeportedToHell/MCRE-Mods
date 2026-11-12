// Each server tick: backpacks to open, closed ones to fill.
fn travelersbackpack_server_tick(game: &mut crate::game::Game<'_>) {
    crate::game::travelersbackpack_glue::server_tick(game);
}
