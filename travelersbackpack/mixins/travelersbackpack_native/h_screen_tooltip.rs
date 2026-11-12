// A backpack screen's tooltips.
fn travelersbackpack_screen_tooltip(
    game: &crate::game::Game<'_>,
    block: &str,
    data: &[i32],
    at: [f32; 2],
    lines: &mut Vec<String>,
) {
    crate::game::travelersbackpack_glue::screen_tooltip(game, block, data, at, lines);
}
