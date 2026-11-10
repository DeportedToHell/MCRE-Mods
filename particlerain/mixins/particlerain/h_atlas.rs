// The atlas is being packed: Particle Rain's sprites join its images.
fn particlerain_atlas(images: &mut Vec<(String, u32, u32, Vec<u8>)>) {
    crate::hud::particlerain_atlas::add_images(images);
}
