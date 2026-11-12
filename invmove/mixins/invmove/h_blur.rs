// The menu blur behind a screen: none where InvMove hides the background.
fn invmove_blur(radius: &mut Option<f32>) {
    crate::game::invmove_glue::menu_blur(radius);
}
