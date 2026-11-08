// The mod's settings page: a button per option.
fn pickupnotifier_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::pickupnotifier_glue::page(game, mod_id, buttons);
}
