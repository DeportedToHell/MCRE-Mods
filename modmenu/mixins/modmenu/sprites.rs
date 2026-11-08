// Mod Menu's textures (MIT, https://github.com/TerraformersMC/ModMenu,
// src/main/resources/assets/modmenu), as runs
// of pixels: (x, y, length, colour as the game's rgba() packs it). Made by
// tools/sprites.mjs; don't edit by hand.

pub(super) struct Sprite {
    pub(super) width: u16,
    pub(super) runs: &'static [(u8, u8, u8, u32)],
}

/// icon.png, 16x16.
pub(super) const ICON: Sprite = Sprite {
    width: 16,
    runs: &[(0,0,16,0xffff4b13),(0,1,16,0xffff4b13),(0,2,2,0xffff4b13),(2,2,10,0xffffffff),(12,2,1,0xffff4b13),(13,2,1,0xffffffff),(14,2,2,0xffff4b13),(0,3,16,0xffff4b13),(0,4,2,0xffff4b13),(2,4,2,0xffffffff),(4,4,1,0xffff4b13),(5,4,9,0xffffffff),(14,4,2,0xffff4b13),(0,5,2,0xffff4b13),(2,5,2,0xffffffff),(4,5,1,0xffff4b13),(5,5,9,0xffffffff),(14,5,2,0xffff4b13),(0,6,16,0xffff4b13),(0,7,2,0xffff4b13),(2,7,2,0xffffffff),(4,7,1,0xffff4b13),(5,7,9,0xffffffff),(14,7,2,0xffff4b13),(0,8,2,0xffff4b13),(2,8,2,0xffffffff),(4,8,1,0xffff4b13),(5,8,9,0xffffffff),(14,8,2,0xffff4b13),(0,9,16,0xffff4b13),(0,10,2,0xffff4b13),(2,10,2,0xffffffff),(4,10,1,0xffff4b13),(5,10,9,0xffffffff),(14,10,2,0xffff4b13),(0,11,2,0xffff4b13),(2,11,2,0xffffffff),(4,11,1,0xffff4b13),(5,11,9,0xffffffff),(14,11,2,0xffff4b13),(0,12,16,0xffff4b13),(0,13,2,0xffff4b13),(2,13,5,0xffffffff),(7,13,2,0xffff4b13),(9,13,5,0xffffffff),(14,13,2,0xffff4b13),(0,14,16,0xffff4b13),(0,15,16,0xffff4b13)],
};

/// unknown_icon.png, 16x16.
pub(super) const UNKNOWN: Sprite = Sprite {
    width: 16,
    runs: &[(0,0,16,0xff585858),(0,1,16,0xff585858),(0,2,4,0xff585858),(4,2,8,0xffffffff),(12,2,4,0xff585858),(0,3,4,0xff585858),(4,3,8,0xffffffff),(12,3,4,0xff585858),(0,4,4,0xff585858),(4,4,2,0xffffffff),(6,4,4,0xff585858),(10,4,2,0xffffffff),(12,4,4,0xff585858),(0,5,4,0xff585858),(4,5,2,0xffffffff),(6,5,4,0xff585858),(10,5,2,0xffffffff),(12,5,4,0xff585858),(0,6,10,0xff585858),(10,6,2,0xffffffff),(12,6,4,0xff585858),(0,7,7,0xff585858),(7,7,5,0xffffffff),(12,7,4,0xff585858),(0,8,7,0xff585858),(7,8,5,0xffffffff),(12,8,4,0xff585858),(0,9,7,0xff585858),(7,9,2,0xffffffff),(9,9,7,0xff585858),(0,10,7,0xff585858),(7,10,2,0xffffffff),(9,10,7,0xff585858),(0,11,16,0xff585858),(0,12,7,0xff585858),(7,12,2,0xffffffff),(9,12,7,0xff585858),(0,13,7,0xff585858),(7,13,2,0xffffffff),(9,13,7,0xff585858),(0,14,16,0xff585858),(0,15,16,0xff585858)],
};

/// minecraft_icon.png, 9x9.
pub(super) const MINECRAFT: Sprite = Sprite {
    width: 9,
    runs: &[(0,0,1,0xff5dbf76),(1,0,1,0xff4db462),(2,0,1,0xff50bc6f),(3,0,1,0xff5dbf76),(4,0,1,0xff50bc6f),(5,0,1,0xff4db363),(6,0,1,0xff4caf5e),(7,0,1,0xff4dbb6a),(8,0,1,0xff4db462),(0,1,1,0xff4db361),(1,1,1,0xff2e4566),(2,1,1,0xff4db361),(3,1,1,0xff4ba151),(4,1,1,0xff4caa57),(5,1,1,0xff304666),(6,1,1,0xff4ba151),(7,1,1,0xff2d4465),(8,1,1,0xff50bc6f),(0,2,1,0xff2e4566),(1,2,1,0xff6d6d6c),(2,2,1,0xff2e4565),(3,2,1,0xff2d4465),(4,2,1,0xff4db361),(5,2,3,0xff2e4566),(8,2,1,0xff5dbf76),(0,3,1,0xff496b97),(1,3,1,0xff355078),(2,3,1,0xff496b97),(3,3,1,0xff5c83be),(4,3,1,0xff2e4566),(5,3,1,0xff5c83be),(6,3,2,0xff496b97),(8,3,1,0xff2e4566),(0,4,1,0xff355078),(1,4,1,0xff2e4566),(2,4,2,0xff496b97),(4,4,1,0xff355078),(5,4,1,0xff5c83be),(6,4,3,0xff2e4566),(0,5,1,0xff5c83be),(1,5,3,0xff355078),(4,5,1,0xff72716c),(5,5,2,0xff355078),(7,5,2,0xff5c83be),(0,6,2,0xff355078),(2,6,2,0xff5c83be),(4,6,2,0xff496b97),(6,6,2,0xff355078),(8,6,1,0xff496b97),(0,7,1,0xff496b97),(1,7,2,0xff355078),(3,7,1,0xff496b97),(4,7,1,0xff355078),(5,7,1,0xff496b97),(6,7,1,0xff355078),(7,7,1,0xff2e4566),(8,7,1,0xff355078),(0,8,1,0xff355078),(1,8,1,0xff496b97),(2,8,1,0xff2e4566),(3,8,2,0xff355078),(5,8,2,0xff2e4566),(7,8,2,0xff355078)],
};


/// textures/gui/sprites/filters/enabled.png, 20x20.
pub(super) const FILTERS: Sprite = Sprite {
    width: 20,
    runs: &[(4,4,12,0xffffffff),(5,5,2,0xffffffff),(7,5,6,0xff3f3f3f),(13,5,2,0xffffffff),(15,5,2,0xff3f3f3f),(6,6,2,0xffffffff),(12,6,2,0xffffffff),(14,6,2,0xff3f3f3f),(7,7,2,0xffffffff),(11,7,2,0xffffffff),(13,7,2,0xff3f3f3f),(8,8,4,0xffffffff),(12,8,2,0xff3f3f3f),(9,9,2,0xffffffff),(11,9,2,0xff3f3f3f),(9,10,2,0xffffffff),(11,10,1,0xff3f3f3f),(9,11,2,0xffffffff),(11,11,1,0xff3f3f3f),(9,12,2,0xffffffff),(11,12,1,0xff3f3f3f),(9,13,2,0xffffffff),(11,13,1,0xff3f3f3f),(9,14,1,0xffffffff),(10,14,2,0xff3f3f3f),(10,15,1,0xff3f3f3f)],
};

/// textures/gui/sprites/configure/enabled.png, 20x20.
pub(super) const CONFIGURE: Sprite = Sprite {
    width: 20,
    runs: &[(6,3,3,0xffffffff),(4,4,3,0xffffffff),(7,4,1,0xff3f3f3f),(8,4,8,0xffffffff),(5,5,1,0xff3f3f3f),(6,5,3,0xffffffff),(9,5,8,0xff3f3f3f),(7,6,3,0xff3f3f3f),(10,8,3,0xffffffff),(4,9,7,0xffffffff),(11,9,1,0xff3f3f3f),(12,9,4,0xffffffff),(5,10,5,0xff3f3f3f),(10,10,3,0xffffffff),(13,10,4,0xff3f3f3f),(11,11,3,0xff3f3f3f),(6,13,3,0xffffffff),(4,14,3,0xffffffff),(7,14,1,0xff3f3f3f),(8,14,8,0xffffffff),(5,15,1,0xff3f3f3f),(6,15,3,0xffffffff),(9,15,8,0xff3f3f3f),(7,16,3,0xff3f3f3f)],
};

/// textures/gui/mod_configuration.png (0, 0, 32, 32), 32x32.
pub(super) const QUICK_CONFIGURE: Sprite = Sprite {
    width: 32,
    runs: &[(6,1,10,0xff000000),(6,2,10,0xff000000),(2,3,6,0xff000000),(8,3,4,0xffffffff),(12,3,2,0xff8b8b8b),(14,3,16,0xff000000),(2,4,6,0xff000000),(8,4,4,0xffffffff),(12,4,2,0xff8b8b8b),(14,4,16,0xff000000),(2,5,2,0xff000000),(4,5,6,0xffffffff),(10,5,2,0xff000000),(12,5,14,0xff8b8b8b),(26,5,2,0xff373737),(28,5,2,0xff000000),(2,6,2,0xff000000),(4,6,6,0xffffffff),(10,6,2,0xff000000),(12,6,14,0xff8b8b8b),(26,6,2,0xff373737),(28,6,2,0xff000000),(2,7,6,0xff000000),(8,7,6,0xff8b8b8b),(14,7,16,0xff000000),(2,8,6,0xff000000),(8,8,6,0xff8b8b8b),(14,8,16,0xff000000),(6,9,10,0xff000000),(6,10,10,0xff000000),(14,11,10,0xff000000),(14,12,10,0xff000000),(2,13,14,0xff000000),(16,13,4,0xffffffff),(20,13,2,0xff8b8b8b),(22,13,8,0xff000000),(2,14,14,0xff000000),(16,14,4,0xffffffff),(20,14,2,0xff8b8b8b),(22,14,8,0xff000000),(2,15,2,0xff000000),(4,15,14,0xffffffff),(18,15,2,0xff000000),(20,15,6,0xff8b8b8b),(26,15,2,0xff373737),(28,15,2,0xff000000),(2,16,2,0xff000000),(4,16,14,0xffffffff),(18,16,2,0xff000000),(20,16,6,0xff8b8b8b),(26,16,2,0xff373737),(28,16,2,0xff000000),(2,17,14,0xff000000),(16,17,6,0xff8b8b8b),(22,17,8,0xff000000),(2,18,14,0xff000000),(16,18,6,0xff8b8b8b),(22,18,8,0xff000000),(14,19,10,0xff000000),(14,20,10,0xff000000),(6,21,10,0xff000000),(6,22,10,0xff000000),(2,23,6,0xff000000),(8,23,4,0xffffffff),(12,23,2,0xff8b8b8b),(14,23,16,0xff000000),(2,24,6,0xff000000),(8,24,4,0xffffffff),(12,24,2,0xff8b8b8b),(14,24,16,0xff000000),(2,25,2,0xff000000),(4,25,6,0xffffffff),(10,25,2,0xff000000),(12,25,14,0xff8b8b8b),(26,25,2,0xff373737),(28,25,2,0xff000000),(2,26,2,0xff000000),(4,26,6,0xffffffff),(10,26,2,0xff000000),(12,26,14,0xff8b8b8b),(26,26,2,0xff373737),(28,26,2,0xff000000),(2,27,6,0xff000000),(8,27,6,0xff8b8b8b),(14,27,16,0xff000000),(2,28,6,0xff000000),(8,28,6,0xff8b8b8b),(14,28,16,0xff000000),(6,29,10,0xff000000),(6,30,10,0xff000000)],
};

/// textures/gui/mod_configuration.png (0, 32, 32, 32), 32x32.
pub(super) const QUICK_CONFIGURE_HOVER: Sprite = Sprite {
    width: 32,
    runs: &[(6,1,10,0xff3e0700),(6,2,10,0xff3e0700),(2,3,6,0xff3e0700),(8,3,4,0xffffffff),(12,3,2,0xffc99288),(14,3,16,0xff3e0700),(2,4,6,0xff3e0700),(8,4,4,0xffffffff),(12,4,2,0xffc99288),(14,4,16,0xff3e0700),(2,5,2,0xff3e0700),(4,5,6,0xffffffff),(10,5,2,0xff3e0700),(12,5,14,0xffc99288),(26,5,2,0xff753e34),(28,5,2,0xff3e0700),(2,6,2,0xff3e0700),(4,6,6,0xffffffff),(10,6,2,0xff3e0700),(12,6,14,0xffc99288),(26,6,2,0xff753e34),(28,6,2,0xff3e0700),(2,7,6,0xff3e0700),(8,7,6,0xffc99288),(14,7,16,0xff3e0700),(2,8,6,0xff3e0700),(8,8,6,0xffc99288),(14,8,16,0xff3e0700),(6,9,10,0xff3e0700),(6,10,10,0xff3e0700),(14,11,10,0xff3e0700),(14,12,10,0xff3e0700),(2,13,14,0xff3e0700),(16,13,4,0xffffffff),(20,13,2,0xffc99288),(22,13,8,0xff3e0700),(2,14,14,0xff3e0700),(16,14,4,0xffffffff),(20,14,2,0xffc99288),(22,14,8,0xff3e0700),(2,15,2,0xff3e0700),(4,15,14,0xffffffff),(18,15,2,0xff3e0700),(20,15,6,0xffc99288),(26,15,2,0xff753e34),(28,15,2,0xff3e0700),(2,16,2,0xff3e0700),(4,16,14,0xffffffff),(18,16,2,0xff3e0700),(20,16,6,0xffc99288),(26,16,2,0xff753e34),(28,16,2,0xff3e0700),(2,17,14,0xff3e0700),(16,17,6,0xffc99288),(22,17,8,0xff3e0700),(2,18,14,0xff3e0700),(16,18,6,0xffc99288),(22,18,8,0xff3e0700),(14,19,10,0xff3e0700),(14,20,10,0xff3e0700),(6,21,10,0xff3e0700),(6,22,10,0xff3e0700),(2,23,6,0xff3e0700),(8,23,4,0xffffffff),(12,23,2,0xffc99288),(14,23,16,0xff3e0700),(2,24,6,0xff3e0700),(8,24,4,0xffffffff),(12,24,2,0xffc99288),(14,24,16,0xff3e0700),(2,25,2,0xff3e0700),(4,25,6,0xffffffff),(10,25,2,0xff3e0700),(12,25,14,0xffc99288),(26,25,2,0xff753e34),(28,25,2,0xff3e0700),(2,26,2,0xff3e0700),(4,26,6,0xffffffff),(10,26,2,0xff3e0700),(12,26,14,0xffc99288),(26,26,2,0xff753e34),(28,26,2,0xff3e0700),(2,27,6,0xff3e0700),(8,27,6,0xffc99288),(14,27,16,0xff3e0700),(2,28,6,0xff3e0700),(8,28,6,0xffc99288),(14,28,16,0xff3e0700),(6,29,10,0xff3e0700),(6,30,10,0xff3e0700)],
};
