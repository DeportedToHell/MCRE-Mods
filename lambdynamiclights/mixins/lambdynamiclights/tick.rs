// The end of each client tick: beacons are read again; the Fast modes refresh sources.
fn lambdynamiclights_tick(game: &mut crate::game::Game<'_>) {
    crate::game::lambdynamiclights_glue::tick(game);
}
