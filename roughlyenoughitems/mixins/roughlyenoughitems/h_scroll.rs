// The wheel over a screen: the config screen's panels, the overlays' pages.
fn roughlyenoughitems_scroll(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if !*canceled
        && (crate::game::roughlyenoughitems_options::scrolled(game, screen, scroll)
            || crate::game::roughlyenoughitems_glue::scrolled(game, screen, scroll))
    {
        *canceled = true;
    }
}
