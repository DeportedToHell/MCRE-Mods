// The hose's drink acts on the drinker.
fn travelersbackpack_use_finish(game: &mut crate::game::Game<'_>, stack: &ItemStack) {
    crate::game::travelersbackpack_glue::use_item_finish(game, stack);
}
