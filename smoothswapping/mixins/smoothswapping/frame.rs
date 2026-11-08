// Each frame: its time, for the flights.
fn smoothswapping_frame(elapsed: std::time::Duration) {
    crate::game::smoothswapping_glue::frame(elapsed);
}
