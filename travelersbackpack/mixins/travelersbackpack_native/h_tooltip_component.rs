// A backpack's contents under its tooltip while Ctrl is held: its size.
fn travelersbackpack_tooltip_component(game: &crate::game::Game<'_>, stack: &ItemStack, size: &mut Option<[f32; 2]>) {
    crate::game::travelersbackpack_glue::tooltip_component(game, stack, size);
}
