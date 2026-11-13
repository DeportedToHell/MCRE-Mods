// `/tb` for operators (`AccessCommand`, `ClearCommand`, `RestoreCommand`, `UnpackCommand`).
fn travelersbackpack_register_commands(commands: &mut Vec<horizon_client::mod_api::NativeCommand>) {
    crate::game::travelersbackpack_glue::register_commands(commands);
}
