// The blaze backpack: no fall; the creeper backpack: its own blast spares its wearer.
fn travelersbackpack_living_incoming_damage(entity: i32, kind: &str, damage_type: &str, amount: &mut f32, canceled: &mut bool) {
    crate::game::travelersbackpack_glue::living_incoming_damage(entity, kind, damage_type, *amount, canceled);
}
