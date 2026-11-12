// The wheel over a filter's tag list moves its selection.
fn travelersbackpack_screen_scrolled(screen: &horizon_client::mod_api::NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    crate::game::travelersbackpack_glue::tag_scroll(screen, scroll, canceled);
}
