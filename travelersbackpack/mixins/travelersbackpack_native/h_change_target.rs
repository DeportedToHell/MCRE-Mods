// Ghasts leave the ghast backpack be; endermen aren't angered by the pumpkin backpack.
fn travelersbackpack_change_target(entity: u32, kind: &str, target: Option<u128>, cancel: &mut bool) {
    crate::game::travelersbackpack_glue::living_change_target(entity, kind, target, cancel);
}
