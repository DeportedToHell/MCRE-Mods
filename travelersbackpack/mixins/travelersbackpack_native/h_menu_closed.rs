// A backpack's menu closed: its items go back into the backpack.
fn travelersbackpack_menu_closed(uuid: u128, window: i32, slots: &mut Vec<Option<horizon_client::items::ItemStack>>) {
    crate::game::travelersbackpack_glue::menu_closed(uuid, window, slots);
}
