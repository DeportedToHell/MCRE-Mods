// A key on any screen: an operation's key over an inventory.
fn clientsort_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    if game.native_on(crate::game::clientsort::ID) {
        crate::game::clientsort_glue::key(game, key, pressed);
    }
}
