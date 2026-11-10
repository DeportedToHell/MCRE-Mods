// Emotecraft's emote icons in the game's atlas: room for them, made while the atlas is packed
// (`emotecraft:icon/slot<n>`, 128 x 128 each), which the game fills with an emote's icon at run
// time as it does a map's (`map_patches`). No room of the shared mod-image slot is used.
// Spliced into horizon-client-render's hud.rs as `emotecraft_atlas`.
//
// This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.

/// How many icons show at once: a screen's list rows and the wheel's tiles.
pub const SLOTS: usize = 24;
pub const SIZE: u32 = 128;

/// A slot's sprite name.
pub fn sprite(i: usize) -> String {
    format!("emotecraft:icon/slot{i}")
}

/// HEAD of `HudAtlas::slots`: the empty slots.
pub fn add(images: &mut Vec<(String, u32, u32, Vec<u8>)>) {
    for i in 0..SLOTS {
        images.push((sprite(i), SIZE, SIZE, vec![0; (SIZE * SIZE * 4) as usize]));
    }
}
