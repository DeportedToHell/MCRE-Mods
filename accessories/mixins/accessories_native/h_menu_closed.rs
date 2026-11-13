// An accessories menu closed: its items go back to the player at the tick.
fn accessories_menu_closed(uuid: u128, window: i32, slots: &mut Vec<Option<horizon_client::items::ItemStack>>) {
    crate::game::accessories_glue::menu_closed(uuid, window, slots);
}
