// A character typed on a screen: the config screen's field takes it while it
// is up; one a mod's field took (cancelled) means that field has the keyboard.
fn invmove_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if crate::game::invmove_screen::char_typed(game, screen, c) {
        *canceled = true;
    }
    if *canceled && !c.is_control() {
        crate::game::invmove_glue::mod_typed();
    }
}
