// A player respawned: everyone is told again.
fn accessories_respawned(player: u128, name: &str) {
    let _ = name;
    crate::game::accessories_glue::logged_in(player);
}
