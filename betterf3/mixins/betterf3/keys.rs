// BetterF3's debug keys (`KeyboardMixin.processF3`): F3+M opens its config
// screen, F3+F cycles the render distance (Ctrl: the simulation distance).
// `handleDebugKeys` uses them up here, so the key goes no further and F3's
// release doesn't toggle the overlay; the game runs them at its next
// frame (`take`). Ported from BetterF3 by cominixo and TreyRuffy (MIT).
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// The mod is switched on in Options > Mods (the game keeps it current).
pub static ON: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicU8 = AtomicU8::new(0);

pub const CONFIG: u8 = 1;
pub const DISTANCE: u8 = 2;

/// F3 held and `key` pressed: true when BetterF3 takes it.
pub fn handle(key: &str) -> bool {
    if !ON.load(Ordering::Relaxed) {
        return false;
    }
    let action = match key {
        "key.keyboard.m" => CONFIG,
        "key.keyboard.f" => DISTANCE,
        _ => return false,
    };
    PENDING.store(action, Ordering::Relaxed);
    true
}

/// The key taken since the last call, if any.
pub fn take() -> u8 {
    PENDING.swap(0, Ordering::Relaxed)
}
