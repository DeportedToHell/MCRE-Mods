// Cloth Config's texture (LGPL-3.0, https://github.com/shedaniel/cloth-config,
// assets/cloth-config2/textures/gui/cloth_config.png), as runs
// of pixels: (x, y, length, colour as the game's rgba() packs it). Made by
// tools/sprites.mjs; don't edit by hand.

pub(super) struct Sprite {
    pub(super) width: u16,
    pub(super) runs: &'static [(u8, u8, u8, u32)],
}

/// cloth_config.png (12, 0, 12, 18), 12x18.
pub(super) const TAB_LEFT: Sprite = Sprite {
    width: 12,
    runs: &[(8,0,4,0xff000000),(8,1,4,0xff000000),(6,2,2,0xff000000),(8,2,2,0xff2c2c2c),(10,2,2,0xff000000),(6,3,2,0xff000000),(8,3,1,0xff2d2d2d),(9,3,1,0xff2a2a2a),(10,3,2,0xff000000),(4,4,2,0xff000000),(6,4,2,0xff2c2c2c),(8,4,1,0xff2b2b2b),(9,4,1,0xff2a2a2a),(10,4,2,0xff000000),(4,5,2,0xff000000),(6,5,4,0xff2c2c2c),(10,5,2,0xff000000),(2,6,2,0xff000000),(4,6,1,0xff292929),(5,6,1,0xff2d2d2d),(6,6,1,0xff2a2a2a),(7,6,1,0xff262626),(8,6,1,0xff2a2a2a),(9,6,1,0xff2c2c2c),(10,6,2,0xff000000),(2,7,2,0xff000000),(4,7,1,0xff2b2b2b),(5,7,3,0xff2c2c2c),(8,7,1,0xff2b2b2b),(9,7,1,0xff292929),(10,7,2,0xff000000),(0,8,2,0xff000000),(2,8,1,0xff2b2b2b),(3,8,2,0xff2a2a2a),(5,8,2,0xff292929),(7,8,1,0xff2a2a2a),(8,8,2,0xff2c2c2c),(10,8,2,0xff000000),(0,9,2,0xff000000),(2,9,2,0xff2c2c2c),(4,9,1,0xff2a2a2a),(5,9,2,0xff2c2c2c),(7,9,1,0xff2d2d2d),(8,9,1,0xff313131),(9,9,1,0xff2d2d2d),(10,9,2,0xff000000),(2,10,2,0xff000000),(4,10,1,0xff2a2a2a),(5,10,1,0xff2c2c2c),(6,10,1,0xff2e2e2e),(7,10,2,0xff2c2c2c),(9,10,1,0xff2e2e2e),(10,10,2,0xff000000),(2,11,2,0xff000000),(4,11,1,0xff2d2d2d),(5,11,1,0xff2e2e2e),(6,11,3,0xff2c2c2c),(9,11,1,0xff2e2e2e),(10,11,2,0xff000000),(4,12,2,0xff000000),(6,12,4,0xff2c2c2c),(10,12,2,0xff000000),(4,13,2,0xff000000),(6,13,3,0xff2c2c2c),(9,13,1,0xff2f2f2f),(10,13,2,0xff000000),(6,14,2,0xff000000),(8,14,1,0xff2c2c2c),(9,14,1,0xff2a2a2a),(10,14,2,0xff000000),(6,15,2,0xff000000),(8,15,1,0xff2d2d2d),(9,15,1,0xff2c2c2c),(10,15,2,0xff000000),(8,16,4,0xff000000),(8,17,4,0xff000000)],
};

/// cloth_config.png (12, 18, 12, 18), 12x18.
pub(super) const TAB_LEFT_ACTIVE: Sprite = Sprite {
    width: 12,
    runs: &[(8,0,4,0xff000000),(8,1,4,0xff000000),(6,2,2,0xff000000),(8,2,2,0xffa6a6a6),(10,2,2,0xff000000),(6,3,2,0xff000000),(8,3,2,0xffa6a6a6),(10,3,2,0xff000000),(4,4,2,0xff000000),(6,4,2,0xffa6a6a6),(8,4,2,0xff6f6f6f),(10,4,2,0xff000000),(4,5,2,0xff000000),(6,5,2,0xffa6a6a6),(8,5,2,0xff6f6f6f),(10,5,2,0xff000000),(2,6,2,0xff000000),(4,6,2,0xffa6a6a6),(6,6,4,0xff6f6f6f),(10,6,2,0xff000000),(2,7,2,0xff000000),(4,7,2,0xffa6a6a6),(6,7,4,0xff6f6f6f),(10,7,2,0xff000000),(0,8,2,0xff000000),(2,8,8,0xff6f6f6f),(10,8,2,0xff000000),(0,9,2,0xff000000),(2,9,8,0xff6f6f6f),(10,9,2,0xff000000),(2,10,2,0xff000000),(4,10,2,0xff565656),(6,10,4,0xff6f6f6f),(10,10,2,0xff000000),(2,11,2,0xff000000),(4,11,2,0xff565656),(6,11,4,0xff6f6f6f),(10,11,2,0xff000000),(4,12,2,0xff000000),(6,12,2,0xff565656),(8,12,2,0xff6f6f6f),(10,12,2,0xff000000),(4,13,2,0xff000000),(6,13,2,0xff565656),(8,13,2,0xff6f6f6f),(10,13,2,0xff000000),(6,14,2,0xff000000),(8,14,2,0xff565656),(10,14,2,0xff000000),(6,15,2,0xff000000),(8,15,2,0xff565656),(10,15,2,0xff000000),(8,16,4,0xff000000),(8,17,4,0xff000000)],
};

/// cloth_config.png (12, 36, 12, 18), 12x18.
pub(super) const TAB_LEFT_HOVER: Sprite = Sprite {
    width: 12,
    runs: &[(8,0,4,0xff000000),(8,1,4,0xff000000),(6,2,2,0xff000000),(8,2,2,0xffffc7be),(10,2,2,0xff000000),(6,3,2,0xff000000),(8,3,2,0xffffc7be),(10,3,2,0xff000000),(4,4,2,0xff000000),(6,4,2,0xffffc7be),(8,4,2,0xffc18c82),(10,4,2,0xff000000),(4,5,2,0xff000000),(6,5,2,0xffffc7be),(8,5,2,0xffc18c82),(10,5,2,0xff000000),(2,6,2,0xff000000),(4,6,2,0xffffc7be),(6,6,4,0xffc18c82),(10,6,2,0xff000000),(2,7,2,0xff000000),(4,7,2,0xffffc7be),(6,7,4,0xffc18c82),(10,7,2,0xff000000),(0,8,2,0xff000000),(2,8,8,0xffc18c82),(10,8,2,0xff000000),(0,9,2,0xff000000),(2,9,8,0xffc18c82),(10,9,2,0xff000000),(2,10,2,0xff000000),(4,10,2,0xffa66f64),(6,10,4,0xffc18c82),(10,10,2,0xff000000),(2,11,2,0xff000000),(4,11,2,0xffa66f64),(6,11,4,0xffc18c82),(10,11,2,0xff000000),(4,12,2,0xff000000),(6,12,2,0xffa66f64),(8,12,2,0xffc18c82),(10,12,2,0xff000000),(4,13,2,0xff000000),(6,13,2,0xffa66f64),(8,13,2,0xffc18c82),(10,13,2,0xff000000),(6,14,2,0xff000000),(8,14,2,0xffa66f64),(10,14,2,0xff000000),(6,15,2,0xff000000),(8,15,2,0xffa66f64),(10,15,2,0xff000000),(8,16,4,0xff000000),(8,17,4,0xff000000)],
};

/// cloth_config.png (0, 0, 12, 18), 12x18.
pub(super) const TAB_RIGHT: Sprite = Sprite {
    width: 12,
    runs: &[(0,0,4,0xff000000),(0,1,4,0xff000000),(0,2,2,0xff000000),(2,2,1,0xff2d2d2d),(3,2,1,0xff303030),(4,2,2,0xff000000),(0,3,2,0xff000000),(2,3,1,0xff2a2a2a),(3,3,1,0xff2c2c2c),(4,3,2,0xff000000),(0,4,2,0xff000000),(2,4,1,0xff292929),(3,4,1,0xff2b2b2b),(4,4,1,0xff2a2a2a),(5,4,1,0xff262626),(6,4,2,0xff000000),(0,5,2,0xff000000),(2,5,1,0xff2c2c2c),(3,5,1,0xff2f2f2f),(4,5,2,0xff2c2c2c),(6,5,2,0xff000000),(0,6,2,0xff000000),(2,6,1,0xff333333),(3,6,1,0xff2e2e2e),(4,6,2,0xff2c2c2c),(6,6,1,0xff2b2b2b),(7,6,1,0xff2c2c2c),(8,6,2,0xff000000),(0,7,2,0xff000000),(2,7,1,0xff2a2a2a),(3,7,2,0xff2c2c2c),(5,7,1,0xff2d2d2d),(6,7,1,0xff2c2c2c),(7,7,1,0xff2a2a2a),(8,7,2,0xff000000),(0,8,2,0xff000000),(2,8,1,0xff2c2c2c),(3,8,2,0xff2a2a2a),(5,8,1,0xff2c2c2c),(6,8,1,0xff2b2b2b),(7,8,1,0xff2f2f2f),(8,8,2,0xff2d2d2d),(10,8,2,0xff000000),(0,9,2,0xff000000),(2,9,1,0xff2f2f2f),(3,9,3,0xff2c2c2c),(6,9,1,0xff2d2d2d),(7,9,1,0xff313131),(8,9,1,0xff2c2c2c),(9,9,1,0xff262626),(10,9,2,0xff000000),(0,10,2,0xff000000),(2,10,6,0xff2c2c2c),(8,10,2,0xff000000),(0,11,2,0xff000000),(2,11,1,0xff2c2c2c),(3,11,1,0xff2f2f2f),(4,11,4,0xff2c2c2c),(8,11,2,0xff000000),(0,12,2,0xff000000),(2,12,1,0xff282828),(3,12,1,0xff2a2a2a),(4,12,2,0xff2c2c2c),(6,12,2,0xff000000),(0,13,2,0xff000000),(2,13,3,0xff2c2c2c),(5,13,1,0xff2b2b2b),(6,13,2,0xff000000),(0,14,2,0xff000000),(2,14,1,0xff2a2a2a),(3,14,1,0xff282828),(4,14,2,0xff000000),(0,15,2,0xff000000),(2,15,1,0xff2c2c2c),(3,15,1,0xff2a2a2a),(4,15,2,0xff000000),(0,16,4,0xff000000),(0,17,4,0xff000000)],
};

/// cloth_config.png (0, 18, 12, 18), 12x18.
pub(super) const TAB_RIGHT_ACTIVE: Sprite = Sprite {
    width: 12,
    runs: &[(0,0,4,0xff000000),(0,1,4,0xff000000),(0,2,2,0xff000000),(2,2,2,0xffa6a6a6),(4,2,2,0xff000000),(0,3,2,0xff000000),(2,3,2,0xffa6a6a6),(4,3,2,0xff000000),(0,4,2,0xff000000),(2,4,4,0xffa6a6a6),(6,4,2,0xff000000),(0,5,2,0xff000000),(2,5,4,0xffa6a6a6),(6,5,2,0xff000000),(0,6,2,0xff000000),(2,6,2,0xffa6a6a6),(4,6,2,0xff6f6f6f),(6,6,2,0xffa6a6a6),(8,6,2,0xff000000),(0,7,2,0xff000000),(2,7,2,0xffa6a6a6),(4,7,2,0xff6f6f6f),(6,7,2,0xffa6a6a6),(8,7,2,0xff000000),(0,8,2,0xff000000),(2,8,2,0xffa6a6a6),(4,8,4,0xff6f6f6f),(8,8,2,0xff565656),(10,8,2,0xff000000),(0,9,2,0xff000000),(2,9,2,0xffa6a6a6),(4,9,4,0xff6f6f6f),(8,9,2,0xff565656),(10,9,2,0xff000000),(0,10,2,0xff000000),(2,10,4,0xff6f6f6f),(6,10,2,0xff565656),(8,10,2,0xff000000),(0,11,2,0xff000000),(2,11,4,0xff6f6f6f),(6,11,2,0xff565656),(8,11,2,0xff000000),(0,12,2,0xff000000),(2,12,2,0xff6f6f6f),(4,12,2,0xff565656),(6,12,2,0xff000000),(0,13,2,0xff000000),(2,13,2,0xff6f6f6f),(4,13,2,0xff565656),(6,13,2,0xff000000),(0,14,2,0xff000000),(2,14,2,0xff565656),(4,14,2,0xff000000),(0,15,2,0xff000000),(2,15,2,0xff565656),(4,15,2,0xff000000),(0,16,4,0xff000000),(0,17,4,0xff000000)],
};

/// cloth_config.png (0, 36, 12, 18), 12x18.
pub(super) const TAB_RIGHT_HOVER: Sprite = Sprite {
    width: 12,
    runs: &[(0,0,4,0xff000000),(0,1,4,0xff000000),(0,2,2,0xff000000),(2,2,2,0xffffc7be),(4,2,2,0xff000000),(0,3,2,0xff000000),(2,3,2,0xffffc7be),(4,3,2,0xff000000),(0,4,2,0xff000000),(2,4,4,0xffffc7be),(6,4,2,0xff000000),(0,5,2,0xff000000),(2,5,4,0xffffc7be),(6,5,2,0xff000000),(0,6,2,0xff000000),(2,6,2,0xffffc7be),(4,6,2,0xffc18c82),(6,6,2,0xffffc7be),(8,6,2,0xff000000),(0,7,2,0xff000000),(2,7,2,0xffffc7be),(4,7,2,0xffc18c82),(6,7,2,0xffffc7be),(8,7,2,0xff000000),(0,8,2,0xff000000),(2,8,2,0xffffc7be),(4,8,4,0xffc18c82),(8,8,2,0xffa66f64),(10,8,2,0xff000000),(0,9,2,0xff000000),(2,9,2,0xffffc7be),(4,9,4,0xffc18c82),(8,9,2,0xffa66f64),(10,9,2,0xff000000),(0,10,2,0xff000000),(2,10,4,0xffc18c82),(6,10,2,0xffa66f64),(8,10,2,0xff000000),(0,11,2,0xff000000),(2,11,4,0xffc18c82),(6,11,2,0xffa66f64),(8,11,2,0xff000000),(0,12,2,0xff000000),(2,12,2,0xffc18c82),(4,12,2,0xffa66f64),(6,12,2,0xff000000),(0,13,2,0xff000000),(2,13,2,0xffc18c82),(4,13,2,0xffa66f64),(6,13,2,0xff000000),(0,14,2,0xff000000),(2,14,2,0xffa66f64),(4,14,2,0xff000000),(0,15,2,0xff000000),(2,15,2,0xffa66f64),(4,15,2,0xff000000),(0,16,4,0xff000000),(0,17,4,0xff000000)],
};

/// cloth_config.png (0, 54, 3, 11), 3x11.
pub(super) const ERROR: Sprite = Sprite {
    width: 3,
    runs: &[(0,0,3,0xff1a1a50),(0,1,1,0xff1a1a50),(1,1,1,0xfffffffc),(2,1,1,0xff1a1a50),(0,2,1,0xff1a1a50),(1,2,1,0xff5555ff),(2,2,1,0xff1a1a50),(0,3,1,0xff1a1a50),(1,3,1,0xff5555ff),(2,3,1,0xff1a1a50),(0,4,1,0xff1a1a50),(1,4,1,0xff5555ff),(2,4,1,0xff1a1a50),(0,5,1,0xff1a1a50),(1,5,1,0xff5555ff),(2,5,1,0xff1a1a50),(0,6,1,0xff1a1a50),(1,6,1,0xff5555ff),(2,6,1,0xff1a1a50),(0,7,1,0xff1a1a50),(1,7,1,0xff2a2a80),(2,7,1,0xff1a1a50),(0,8,3,0xff1a1a50),(0,9,1,0xff1a1a50),(1,9,1,0xff5555ff),(2,9,1,0xff1a1a50),(0,10,3,0xff1a1a50)],
};

/// cloth_config.png (33, 0, 9, 9), 9x9.
pub(super) const EXPAND: Sprite = Sprite {
    width: 9,
    runs: &[(3,2,1,0xffffffff),(4,3,1,0xffffffff),(5,4,1,0xffffffff),(4,5,1,0xffffffff),(3,6,1,0xffffffff)],
};

/// cloth_config.png (33, 9, 9, 9), 9x9.
pub(super) const EXPAND_OPEN: Sprite = Sprite {
    width: 9,
    runs: &[(2,3,1,0xffffffff),(6,3,1,0xffffffff),(3,4,1,0xffffffff),(5,4,1,0xffffffff),(4,5,1,0xffffffff)],
};

/// cloth_config.png (33, 18, 9, 9), 9x9.
pub(super) const EXPAND_HOVER: Sprite = Sprite {
    width: 9,
    runs: &[(3,2,1,0xff16fee6),(4,3,1,0xff16fee6),(5,4,1,0xff16fee6),(4,5,1,0xff16fee6),(3,6,1,0xff16fee6)],
};

/// cloth_config.png (33, 27, 9, 9), 9x9.
pub(super) const EXPAND_OPEN_HOVER: Sprite = Sprite {
    width: 9,
    runs: &[(2,3,1,0xff16fee6),(6,3,1,0xff16fee6),(3,4,1,0xff16fee6),(5,4,1,0xff16fee6),(4,5,1,0xff16fee6)],
};

/// cloth_config.png (42, 0, 9, 9), 9x9.
pub(super) const INSERT: Sprite = Sprite {
    width: 9,
    runs: &[(3,1,3,0xffffffff),(2,2,1,0xffffffff),(6,2,1,0xffffffff),(1,3,1,0xffffffff),(4,3,1,0xffffffff),(7,3,1,0xffffffff),(1,4,1,0xffffffff),(3,4,3,0xffffffff),(7,4,1,0xffffffff),(1,5,1,0xffffffff),(4,5,1,0xffffffff),(7,5,1,0xffffffff),(2,6,1,0xffffffff),(6,6,1,0xffffffff),(3,7,3,0xffffffff)],
};

/// cloth_config.png (42, 9, 9, 9), 9x9.
pub(super) const INSERT_HOVER: Sprite = Sprite {
    width: 9,
    runs: &[(3,1,3,0xff16fee6),(2,2,1,0xff16fee6),(6,2,1,0xff16fee6),(1,3,1,0xff16fee6),(4,3,1,0xff16fee6),(7,3,1,0xff16fee6),(1,4,1,0xff16fee6),(3,4,3,0xff16fee6),(7,4,1,0xff16fee6),(1,5,1,0xff16fee6),(4,5,1,0xff16fee6),(7,5,1,0xff16fee6),(2,6,1,0xff16fee6),(6,6,1,0xff16fee6),(3,7,3,0xff16fee6)],
};

/// cloth_config.png (51, 0, 9, 9), 9x9.
pub(super) const DELETE_OFF: Sprite = Sprite {
    width: 9,
    runs: &[(3,1,3,0xffa7a7a7),(2,2,1,0xffa7a7a7),(6,2,1,0xffa7a7a7),(1,3,1,0xffa7a7a7),(7,3,1,0xffa7a7a7),(1,4,1,0xffa7a7a7),(3,4,3,0xffa7a7a7),(7,4,1,0xffa7a7a7),(1,5,1,0xffa7a7a7),(7,5,1,0xffa7a7a7),(2,6,1,0xffa7a7a7),(6,6,1,0xffa7a7a7),(3,7,3,0xffa7a7a7)],
};

/// cloth_config.png (51, 9, 9, 9), 9x9.
pub(super) const DELETE: Sprite = Sprite {
    width: 9,
    runs: &[(3,1,3,0xffffffff),(2,2,1,0xffffffff),(6,2,1,0xffffffff),(1,3,1,0xffffffff),(7,3,1,0xffffffff),(1,4,1,0xffffffff),(3,4,3,0xffffffff),(7,4,1,0xffffffff),(1,5,1,0xffffffff),(7,5,1,0xffffffff),(2,6,1,0xffffffff),(6,6,1,0xffffffff),(3,7,3,0xffffffff)],
};

/// cloth_config.png (51, 18, 9, 9), 9x9.
pub(super) const DELETE_HOVER: Sprite = Sprite {
    width: 9,
    runs: &[(3,1,3,0xff16fee6),(2,2,1,0xff16fee6),(6,2,1,0xff16fee6),(1,3,1,0xff16fee6),(7,3,1,0xff16fee6),(1,4,1,0xff16fee6),(3,4,3,0xff16fee6),(7,4,1,0xff16fee6),(1,5,1,0xff16fee6),(7,5,1,0xff16fee6),(2,6,1,0xff16fee6),(6,6,1,0xff16fee6),(3,7,3,0xff16fee6)],
};
