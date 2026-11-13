// The wheel over a filter's tag list moves its selection; over the tools'
// menu it may pick the hotbar slot.
fn travelersbackpack_screen_scrolled(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    crate::game::travelersbackpack_glue::radial_scroll(game, scroll, canceled);
    crate::game::travelersbackpack_glue::tag_scroll(screen, scroll, canceled);
}
