// A new level: the mod's particles go.
fn particlerain_level_load(dimension: &str) {
    let _ = dimension;
    crate::game::particlerain_glue::level_load();
}
