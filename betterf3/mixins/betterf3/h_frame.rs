// Each frame: the mod's files, the overlay's animation, the screens' requests.
fn betterf3_frame(game: &mut crate::game::Game<'_>) {
    crate::game::betterf3_screens::frame(game);
}
