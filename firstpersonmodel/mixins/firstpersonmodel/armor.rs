// HumanoidArmorLayer, hidden in "Show Layers".
fn firstpersonmodel_armor(value: Vec<(&Part, [f32; 4], u32, u32, bool)>) -> Vec<(&Part, [f32; 4], u32, u32, bool)> {
    if crate::game::firstpersonmodel_glue::armor_hidden() {
        return Vec::new();
    }
    value
}
