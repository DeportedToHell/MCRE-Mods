// Once a frame: the config, joining, the pings (ages, entities, removal), the ping action, the screen.
fn pingwheel_frame(game: &mut crate::game::Game<'_>) {
    crate::game::pingwheel_glue::frame(game);
}
