// The equip and unequip buttons of a backpack's menu.
fn travelersbackpack_menu_button(host: &crate::game::lan::LanHost, conn: u32, window: i32, id: i32) {
    crate::game::travelersbackpack_glue::menu_button(host, conn, window, id);
}
