// A character typed: the config screen's search field, or the search box's
// when it has focus.
fn roughlyenoughitems_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if !*canceled
        && (crate::game::roughlyenoughitems_options::char_typed(game, screen, c)
            || crate::game::roughlyenoughitems_glue::char_typed(game, screen, c))
    {
        *canceled = true;
    }
}
