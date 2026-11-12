// A click on a backpack menu's upgrade or tool slot.
fn travelersbackpack_menu_clicked(game: &mut crate::game::Game<'_>, uuid: u128, window: i32, slot: i32, button: i32, mode: i32) {
    crate::game::travelersbackpack_glue::menu_clicked(game, uuid, window, slot, button, mode);
}
