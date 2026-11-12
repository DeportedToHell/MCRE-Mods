// The key opening the worn backpack.
fn travelersbackpack_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::travelersbackpack_glue::key(game, key, pressed);
}
