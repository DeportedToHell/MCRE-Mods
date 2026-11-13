// Each tick of the window's game: the trash bin's chosen slots to the server,
// the songs of carried jukebox upgrades started.
fn travelersbackpack_client_tick(game: &mut crate::game::Game<'_>) {
    if game.native_window() {
        crate::game::travelersbackpack_glue::send_trash_clicks(game);
        crate::game::travelersbackpack_glue::play_songs(game);
    }
}
