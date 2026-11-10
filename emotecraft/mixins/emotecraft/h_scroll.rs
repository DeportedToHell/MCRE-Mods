// The mouse wheel over Emotecraft's screens: the lists, the emote wheel's pages.
fn emotecraft_scroll(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    if crate::game::emotecraft_ui::scrolled(game, screen, scroll) {
        *canceled = true;
    }
}
