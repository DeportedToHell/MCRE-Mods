// Once a tick: every emote a tick on, the stop rules, the screens' animations.
fn emotecraft_tick(game: &mut crate::game::Game<'_>) {
    crate::game::emotecraft_glue::tick(game);
}
