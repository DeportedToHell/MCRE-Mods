// Once a frame: the config screen's closing and sounds, the data files.
fn pickupnotifier_frame(game: &mut crate::game::Game<'_>) {
    crate::game::pickupnotifier_config::frame(game);
}
