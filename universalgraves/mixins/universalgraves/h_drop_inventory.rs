// The dying player's inventory, before it drops: into their grave.
fn universalgraves_drop_inventory(game: &mut crate::game::Game<'_>, who: (u128, i32)) {
    crate::game::universalgraves::drop_inventory(game, who.0);
}
