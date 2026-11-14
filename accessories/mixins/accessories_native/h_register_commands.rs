// The /accessories commands.
fn accessories_register_commands(commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    crate::game::accessories_glue::register_commands(commands);
}
