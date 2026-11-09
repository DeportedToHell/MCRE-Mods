// Any two orbs may merge (`canMerge`).
fn clumps_can_merge(game: &crate::game::Game<'_>, can_merge: &mut bool) {
    crate::game::clumps_glue::can_merge(game, can_merge);
}