// A player died: their accessories drop at the tick.
fn accessories_player_died(game: &crate::game::Game<'_>, player: u128) {
    crate::game::accessories_glue::player_died(game, player);
}
