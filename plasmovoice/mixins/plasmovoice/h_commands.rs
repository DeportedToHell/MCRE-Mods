// The server's commands: Plasmo Voice's /vlist, /vmute, /vunmute, /vmutelist, /vreload, /vrc.
fn plasmovoice_commands(game: &mut crate::game::Game<'_>, commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    crate::game::plasmovoice_glue::register_commands(game, commands);
}
