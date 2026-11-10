// Once a frame: the emotes, the handshake, the poses, the songs, the screens' requests.
fn emotecraft_frame(game: &mut crate::game::Game<'_>) {
    crate::game::emotecraft_glue::frame(game);
}
