// `spiderAbility` on the client: whether this client's player wears the
// spider backpack with its ability on, which the server says
// (`travelersbackpack:ability`). Movement reads it to climb walls.
use std::sync::atomic::{AtomicBool, Ordering};

static CLIMBS: AtomicBool = AtomicBool::new(false);

pub fn set(on: bool) {
    CLIMBS.store(on, Ordering::Relaxed);
}

pub fn climbs() -> bool {
    CLIMBS.load(Ordering::Relaxed)
}
