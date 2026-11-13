// Nor swapped into the other hand while its menu is open.
fn travelersbackpack_swap_hands(game: &crate::game::Game<'_>, player: u128, cancel: &mut bool) {
    if crate::game::travelersbackpack_glue::hand_menu_open(player) {
        let _ = game;
        *cancel = true;
    }
}
