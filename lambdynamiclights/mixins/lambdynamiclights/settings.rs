// The mod's settings page: a button per option and per listed entity.
fn lambdynamiclights_page(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    buttons: &mut Vec<String>,
) {
    crate::game::lambdynamiclights_glue::page(game, mod_id, buttons);
}
