// InvMove: whether the open screen's in-world menu background is hidden this
// frame (`BackgroundMixin` on `extractBackground`). The game sets it once a
// frame, before drawing; `menu_background` asks it.
use std::sync::atomic::{AtomicBool, Ordering};

static HIDE: AtomicBool = AtomicBool::new(false);

pub fn set(hide: bool) {
    HIDE.store(hide, Ordering::Relaxed);
}

pub fn hidden() -> bool {
    HIDE.load(Ordering::Relaxed)
}
