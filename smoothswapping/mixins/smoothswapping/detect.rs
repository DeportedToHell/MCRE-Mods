// Before the inventory screen draws its slots: what moved since the last
// frame starts flying.
fn smoothswapping_detect(game: &mut crate::game::Game<'_>) {
    crate::game::smoothswapping_glue::detect(game);
}
