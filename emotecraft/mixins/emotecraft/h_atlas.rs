// While the atlas is packed: room for the emote icons.
fn emotecraft_atlas(images: &mut Vec<(String, u32, u32, Vec<u8>)>) {
    crate::hud::emotecraft_atlas::add(images);
}
