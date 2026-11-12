// Each tick of the window's game: the trash bin's chosen slots to the server.
fn travelersbackpack_client_tick(game: &mut crate::game::Game<'_>) {
    if game.native_window() {
        crate::game::travelersbackpack_glue::send_trash_clicks(game);
    }
}
