// Accessories' slot files.
fn universalgraves_reload_apply(listener: &crate::game::native_reload_listeners::NativeReloadListener, files: &[(String, serde_json::Value)]) {
    crate::game::universalgraves::reload_apply(listener, files);
}
