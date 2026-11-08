// Each frame: the running operation's next clicks.
fn clientsort_frame(game: &mut crate::game::Game<'_>, elapsed: std::time::Duration) {
    if game.native_on(crate::game::clientsort::ID) {
        crate::game::clientsort_glue::frame(game, elapsed);
    }
}
