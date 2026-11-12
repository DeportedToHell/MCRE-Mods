// A player died: their worn backpack is placed where they fell.
fn travelersbackpack_player_died(game: &crate::game::Game<'_>, player: u128) {
    crate::game::travelersbackpack_glue::player_died(game, player);
}
