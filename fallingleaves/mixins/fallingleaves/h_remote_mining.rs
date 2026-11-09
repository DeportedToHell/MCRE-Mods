// On a server: a click starts breaking a block (`startDestroyBlock`), a held
// button moves to another, or a creative player breaks the next one.
fn fallingleaves_remote_mining(game: &mut crate::game::Game<'_>, start: bool) {
    let Some(r) = game.remote.as_ref() else {
        return;
    };
    let creative_next = game.creative && r.destroy_delay <= 0;
    let current = r.destroying.map(|(p, _, _)| p);
    let Some(hit) = game.pick() else {
        return;
    };
    if start || creative_next || (!game.creative && current != Some(hit.pos)) {
        crate::game::fallingleaves_glue::hit(game, hit.pos);
    }
}
