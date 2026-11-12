// The dark tint behind an in-game screen: hidden where InvMove hides it.
fn invmove_tint(canceled: &mut bool) {
    crate::game::invmove_glue::transparent_background(canceled);
}
