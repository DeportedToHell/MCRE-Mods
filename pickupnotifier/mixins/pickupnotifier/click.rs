// A click on the settings page: the option's next value (Shift: the previous one).
fn pickupnotifier_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::pickupnotifier_glue::click(game, mod_id, button);
}
