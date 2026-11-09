// The local player starts breaking a block (`startDestroyBlock`).
fn fallingleaves_attack(hands: &mut crate::game::hand::GameHands<'_, '_>, pos: [i32; 3]) {
    crate::game::fallingleaves_glue::hit(&mut *hands.game, pos);
}
