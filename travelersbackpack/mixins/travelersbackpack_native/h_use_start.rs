// No drinking from an empty tank.
fn travelersbackpack_use_start(game: &mut crate::game::Game<'_>, stack: &ItemStack, canceled: &mut bool) {
    crate::game::travelersbackpack_glue::use_item_start(game, stack, canceled);
}
