// A press on a backpack screen's buttons.
fn travelersbackpack_screen_click(block: &str, data: &[i32], at: [f32; 2], right: bool, button: &mut Option<i32>) {
    crate::game::travelersbackpack_glue::screen_click(block, data, at, right, button);
}
