// Cloth Config's texture (LGPL-3.0, https://github.com/shedaniel/cloth-config,
// assets/cloth-config2/textures/gui/cloth_config.png), as runs
// of pixels: (x, y, length, colour as the game's rgba() packs it). Taken from
// ClientSort's port (made there by tools/sprites.mjs); don't edit by hand.

pub(super) struct Sprite {
    pub(super) runs: &'static [(u8, u8, u8, u32)],
}

/// cloth_config.png (0, 54, 3, 11), 3x11.
pub(super) const ERROR: Sprite = Sprite {
    runs: &[(0,0,3,0xff1a1a50),(0,1,1,0xff1a1a50),(1,1,1,0xfffffffc),(2,1,1,0xff1a1a50),(0,2,1,0xff1a1a50),(1,2,1,0xff5555ff),(2,2,1,0xff1a1a50),(0,3,1,0xff1a1a50),(1,3,1,0xff5555ff),(2,3,1,0xff1a1a50),(0,4,1,0xff1a1a50),(1,4,1,0xff5555ff),(2,4,1,0xff1a1a50),(0,5,1,0xff1a1a50),(1,5,1,0xff5555ff),(2,5,1,0xff1a1a50),(0,6,1,0xff1a1a50),(1,6,1,0xff5555ff),(2,6,1,0xff1a1a50),(0,7,1,0xff1a1a50),(1,7,1,0xff2a2a80),(2,7,1,0xff1a1a50),(0,8,3,0xff1a1a50),(0,9,1,0xff1a1a50),(1,9,1,0xff5555ff),(2,9,1,0xff1a1a50),(0,10,3,0xff1a1a50)],
};
