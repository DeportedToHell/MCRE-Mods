// Each frame: the configuration's flags, the ender chest's cache and
// messages; the config screen's clicks, saving, closing and data files.
fn shulkerboxtooltip_frame(game: &mut crate::game::Game<'_>) {
    crate::game::shulkerboxtooltip_glue::frame(game);
    crate::game::shulkerboxtooltip_screen::frame(game);
}
