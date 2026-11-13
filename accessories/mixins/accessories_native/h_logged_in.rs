// A player joined: everyone is told what each wears.
fn accessories_logged_in(player: u128) {
    crate::game::accessories_glue::logged_in(player);
}
