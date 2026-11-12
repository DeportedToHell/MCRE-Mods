// The hose drinking is drawn as drinking.
fn travelersbackpack_use_animation(stack: &ItemStack, animation: &mut UseAnimation) {
    crate::game::travelersbackpack_glue::use_animation(stack, animation);
}
