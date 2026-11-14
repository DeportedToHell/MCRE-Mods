// Accessories' item components, registered before content loads.
fn accessories_component_types(types: &mut Vec<String>) {
    crate::game::accessories_glue::component_types(types);
}
