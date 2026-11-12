// InvMove: the keys the local player's input reads this tick while a screen
// is open (`onInputUpdate`), set by the game at the start of its tick and
// taken where it makes the input's keys (`Keys::with_stick`). None leaves the
// game's own keys.
use std::cell::Cell;

thread_local! {
    static KEYS: Cell<Option<super::Keys>> = const { Cell::new(None) };
}

pub fn set(keys: Option<super::Keys>) {
    KEYS.with(|k| k.set(keys));
}

pub fn take() -> Option<super::Keys> {
    KEYS.with(Cell::take)
}
