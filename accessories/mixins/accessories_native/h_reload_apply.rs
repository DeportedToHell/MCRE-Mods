// A data folder's files.
fn accessories_reload_apply(listener: &crate::game::native_reload_listeners::NativeReloadListener, files: &[(String, serde_json::Value)]) {
    crate::game::accessories_glue::reload_listener_apply(listener, files);
}
