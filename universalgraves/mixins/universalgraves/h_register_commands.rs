// `/graves`.
fn universalgraves_register_commands(commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    crate::game::universalgraves::register_commands(commands);
}
