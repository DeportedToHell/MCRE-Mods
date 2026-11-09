// A creative player breaks a block at once (`startDestroyBlock` in creative).
fn fallingleaves_destroy(hands: &mut crate::game::hand::GameHands<'_, '_>, pos: [i32; 3]) {
    if hands.game.creative {
        crate::game::fallingleaves_glue::hit(&mut *hands.game, pos);
    }
}
