// ShulkerBoxTooltip for Horizon: what the item tooltip code of the client
// library needs to know (the library can't reach the game's modules). The
// game sets these once a frame from the configuration
// (`shulkerboxtooltip_glue::frame`).
use std::sync::atomic::{AtomicBool, Ordering};

/// `ItemContainerContentsMixin`: the vanilla "N x item" lines of a container
/// item go while the tooltip type is not Vanilla.
pub static HIDE_CONTAINER_LINES: AtomicBool = AtomicBool::new(false);
/// `ItemStackMixin.removeLore`: a shulker box's lore goes while "Hide Shulker
/// Box Lore" is on.
pub static HIDE_SHULKER_LORE: AtomicBool = AtomicBool::new(false);

pub fn hide_container_lines() -> bool {
    HIDE_CONTAINER_LINES.load(Ordering::Relaxed)
}

/// Whether `item` (an item id) is a shulker box, plain or dyed (`ShulkerBoxBlock`).
pub fn hide_lore_of(item: &str) -> bool {
    HIDE_SHULKER_LORE.load(Ordering::Relaxed) && item.starts_with("minecraft:") && item.ends_with("shulker_box")
}
