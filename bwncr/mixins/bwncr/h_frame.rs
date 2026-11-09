// Each frame: the config screen's requests, the sound check's settings, and
// what it heard.
fn bwncr_frame(game: &mut crate::game::Game<'_>) {
    crate::game::bwncr_glue::frame(game);
}