// The spyglass's own zoom gives way when Zoomify zooms for it.
fn zoomify_fov_modifier(
    game: &crate::game::Game<'_>,
    player: &horizon_client::camera::FovPlayer,
    modifier: &mut f32,
) {
    crate::game::zoomify_glue::fov_modifier(game, player, modifier);
}
