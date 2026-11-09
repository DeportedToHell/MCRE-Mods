// Cloth Config's texture (LGPL-3.0, https://github.com/shedaniel/cloth-config,
// assets/cloth-config2/textures/gui/cloth_config.png), as runs
// of pixels: (x, y, length, colour as the game's rgba() packs it). Made by
// tools/sprites.mjs; don't edit by hand.

pub(super) struct Sprite {
    pub(super) runs: &'static [(u8, u8, u8, u32)],
}

/// cloth_config.png (24, 0, 9, 9), 9x9.
pub(super) const CATEGORY: Sprite = Sprite {
    runs: &[(4,2,1,0xffffffff),(4,3,1,0xffffffff),(2,4,5,0xffffffff),(4,5,1,0xffffffff),(4,6,1,0xffffffff)],
};

/// cloth_config.png (24, 9, 9, 9), 9x9.
pub(super) const CATEGORY_OPEN: Sprite = Sprite {
    runs: &[(2,4,5,0xffffffff)],
};

/// cloth_config.png (24, 18, 9, 9), 9x9.
pub(super) const CATEGORY_HOVER: Sprite = Sprite {
    runs: &[(4,2,1,0xff16fee6),(4,3,1,0xff16fee6),(2,4,5,0xff16fee6),(4,5,1,0xff16fee6),(4,6,1,0xff16fee6)],
};

/// cloth_config.png (24, 27, 9, 9), 9x9.
pub(super) const CATEGORY_OPEN_HOVER: Sprite = Sprite {
    runs: &[(2,4,5,0xff16fee6)],
};

/// cloth_config.png (33, 0, 9, 9), 9x9.
pub(super) const EXPAND: Sprite = Sprite {
    runs: &[(3,2,1,0xffffffff),(4,3,1,0xffffffff),(5,4,1,0xffffffff),(4,5,1,0xffffffff),(3,6,1,0xffffffff)],
};

/// cloth_config.png (33, 9, 9, 9), 9x9.
pub(super) const EXPAND_OPEN: Sprite = Sprite {
    runs: &[(2,3,1,0xffffffff),(6,3,1,0xffffffff),(3,4,1,0xffffffff),(5,4,1,0xffffffff),(4,5,1,0xffffffff)],
};

/// cloth_config.png (33, 18, 9, 9), 9x9.
pub(super) const EXPAND_HOVER: Sprite = Sprite {
    runs: &[(3,2,1,0xff16fee6),(4,3,1,0xff16fee6),(5,4,1,0xff16fee6),(4,5,1,0xff16fee6),(3,6,1,0xff16fee6)],
};

/// cloth_config.png (33, 27, 9, 9), 9x9.
pub(super) const EXPAND_OPEN_HOVER: Sprite = Sprite {
    runs: &[(2,3,1,0xff16fee6),(6,3,1,0xff16fee6),(3,4,1,0xff16fee6),(5,4,1,0xff16fee6),(4,5,1,0xff16fee6)],
};

/// cloth_config.png (42, 0, 9, 9), 9x9.
pub(super) const INSERT: Sprite = Sprite {
    runs: &[(3,1,3,0xffffffff),(2,2,1,0xffffffff),(6,2,1,0xffffffff),(1,3,1,0xffffffff),(4,3,1,0xffffffff),(7,3,1,0xffffffff),(1,4,1,0xffffffff),(3,4,3,0xffffffff),(7,4,1,0xffffffff),(1,5,1,0xffffffff),(4,5,1,0xffffffff),(7,5,1,0xffffffff),(2,6,1,0xffffffff),(6,6,1,0xffffffff),(3,7,3,0xffffffff)],
};

/// cloth_config.png (42, 9, 9, 9), 9x9.
pub(super) const INSERT_HOVER: Sprite = Sprite {
    runs: &[(3,1,3,0xff16fee6),(2,2,1,0xff16fee6),(6,2,1,0xff16fee6),(1,3,1,0xff16fee6),(4,3,1,0xff16fee6),(7,3,1,0xff16fee6),(1,4,1,0xff16fee6),(3,4,3,0xff16fee6),(7,4,1,0xff16fee6),(1,5,1,0xff16fee6),(4,5,1,0xff16fee6),(7,5,1,0xff16fee6),(2,6,1,0xff16fee6),(6,6,1,0xff16fee6),(3,7,3,0xff16fee6)],
};

/// cloth_config.png (51, 0, 9, 9), 9x9.
pub(super) const DELETE_OFF: Sprite = Sprite {
    runs: &[(3,1,3,0xffa7a7a7),(2,2,1,0xffa7a7a7),(6,2,1,0xffa7a7a7),(1,3,1,0xffa7a7a7),(7,3,1,0xffa7a7a7),(1,4,1,0xffa7a7a7),(3,4,3,0xffa7a7a7),(7,4,1,0xffa7a7a7),(1,5,1,0xffa7a7a7),(7,5,1,0xffa7a7a7),(2,6,1,0xffa7a7a7),(6,6,1,0xffa7a7a7),(3,7,3,0xffa7a7a7)],
};

/// cloth_config.png (51, 9, 9, 9), 9x9.
pub(super) const DELETE: Sprite = Sprite {
    runs: &[(3,1,3,0xffffffff),(2,2,1,0xffffffff),(6,2,1,0xffffffff),(1,3,1,0xffffffff),(7,3,1,0xffffffff),(1,4,1,0xffffffff),(3,4,3,0xffffffff),(7,4,1,0xffffffff),(1,5,1,0xffffffff),(7,5,1,0xffffffff),(2,6,1,0xffffffff),(6,6,1,0xffffffff),(3,7,3,0xffffffff)],
};

/// cloth_config.png (51, 18, 9, 9), 9x9.
pub(super) const DELETE_HOVER: Sprite = Sprite {
    runs: &[(3,1,3,0xff16fee6),(2,2,1,0xff16fee6),(6,2,1,0xff16fee6),(1,3,1,0xff16fee6),(7,3,1,0xff16fee6),(1,4,1,0xff16fee6),(3,4,3,0xff16fee6),(7,4,1,0xff16fee6),(1,5,1,0xff16fee6),(7,5,1,0xff16fee6),(2,6,1,0xff16fee6),(6,6,1,0xff16fee6),(3,7,3,0xff16fee6)],
};
