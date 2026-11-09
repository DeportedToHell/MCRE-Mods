// The merge decided: the orb that takes the other in is worth both.
fn clumps_merge(
    value: (),
    game: &mut crate::game::Game<'_>,
    into: u32,
    from: u32,
    from_value: u32,
    canceled: &mut bool,
) {
    crate::game::clumps_glue::merged(game, into, from, from_value, canceled);
    value
}