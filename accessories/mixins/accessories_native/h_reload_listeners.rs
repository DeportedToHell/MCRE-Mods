// The data folders the mod reads: slots, groups, entity bindings.
fn accessories_reload_listeners(listeners: &mut Vec<crate::game::native_reload_listeners::NativeReloadListener>) {
    crate::game::accessories_glue::add_reload_listeners(listeners);
}
