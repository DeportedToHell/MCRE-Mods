// A key let go: nothing under the mods screen hears it.
fn modmenu_key_released(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::modmenu_glue::covers(game, screen) {
        *canceled = true;
    }
}
