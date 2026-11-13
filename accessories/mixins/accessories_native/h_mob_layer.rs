// Each player's accessories on their model.
fn accessories_mob_layer(
    game: &crate::game::Game<'_>,
    mob: &crate::game::native_mob_draw::NativeMobRender<'_>,
    who: Option<&crate::game::native_mob_draw::NativePlayerRender<'_>>,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    crate::game::accessories_glue::mob_layer(game, mob, who, draw);
}
