// Ping Wheel's texts in the game's language, as a mod's `assets/pingwheel/lang/*.json` would be
// loaded (`ClientLanguage`): English under every language, then the language's own over it; a key
// a resource pack already gives keeps the pack's text. Spliced into horizon-client-core's
// `lang.rs` as `pingwheel_lang`, beside `Language`, whose map it extends.
//
// Ping Wheel by Luken (https://github.com/LukenSkyne/Minecraft-Ping-Wheel), MIT.

use super::{CURRENT, Language};
use std::sync::Arc;

/// Puts `english`, then `local` over it, into the current language unless it holds them already
/// (the first key tells: a reload or another language replaces the whole map).
pub fn ensure(english: &[(&'static str, &'static str)], local: &[(&'static str, &'static str)]) {
    let Some(probe) = english.first().map(|(k, _)| *k) else { return };
    let current = CURRENT.read().unwrap_or_else(|e| e.into_inner()).clone();
    let Some(current) = current else { return };
    if current.map.contains_key(probe) {
        return;
    }
    let mut map = current.map.clone();
    let mut ours: std::collections::HashMap<&str, &str> = english.iter().copied().collect();
    ours.extend(local.iter().copied());
    for (k, v) in ours {
        map.entry(k.to_owned()).or_insert_with(|| v.to_owned());
    }
    let language = Language { code: current.code.clone(), bidirectional: current.bidirectional, map };
    let mut slot = CURRENT.write().unwrap_or_else(|e| e.into_inner());
    // Swapped meanwhile (a reload on another thread): that one is newer, the next call fills it.
    if slot.as_ref().is_some_and(|s| Arc::ptr_eq(s, &current)) {
        *slot = Some(Arc::new(language));
    }
}
