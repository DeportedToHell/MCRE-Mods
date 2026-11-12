// The worn backpack on a player's back.
fn travelersbackpack_mob_layer(
    game: &crate::game::Game<'_>,
    who: Option<&crate::game::native_mob_draw::NativePlayerRender<'_>>,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    crate::game::travelersbackpack_glue::mob_layer(game, who, draw);
}
