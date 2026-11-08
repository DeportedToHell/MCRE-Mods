// A click on the settings page: the option's next value (Shift: the previous one).
fn lambdynamiclights_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::lambdynamiclights_glue::click(game, mod_id, button);
}
