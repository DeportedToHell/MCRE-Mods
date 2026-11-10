// The emotes' icons (`EmoteHolder.getIconIdentifier`, a `DynamicTexture` per emote in Java),
// the folders' and the mod's: decoded once, scaled to 128 x 128, and written into the atlas
// slots the mod adds (`emotecraft:icon/slot<n>`) as the game writes a map's pixels, the least
// recently shown giving way when more show than there are slots.
//
// This Source Code Form is a port of Emotecraft (https://github.com/KosmX/emotes), GPL-3.0.

use super::Game;
use super::emotecraft_glue::lock;
use horizon_client::hud::{HudBatch, Rect};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

const SLOTS: usize = 24;
const SIZE: usize = 128;

/// Icons that aren't an emote's.
pub(super) const FOLDER: u128 = 1;
pub(super) const FOLDER_OPEN: u128 = 2;
pub(super) const LOGO: u128 = 3;

#[derive(Default)]
struct Icons {
    /// Key → its slot and when it last showed.
    slots: HashMap<u128, (usize, u64)>,
    /// Key → its pixels (none if it can't be read).
    pixels: HashMap<u128, Option<Arc<Vec<u8>>>>,
    /// Icons to write, and their bytes.
    wanted: Vec<(u128, Arc<Vec<u8>>)>,
    /// What the atlas was when the slots were written: a new atlas has them empty.
    atlas: Option<(u32, u32, [f32; 4])>,
    clock: u64,
}

static ICONS: Mutex<Option<Icons>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Icons) -> R) -> R {
    let mut g = lock(&ICONS);
    f(g.get_or_insert_with(Icons::default))
}

/// A PNG as 128 x 128 RGBA (`NativeImage.read`, drawn at any size).
fn decode(png_bytes: &[u8]) -> Option<Vec<u8>> {
    let mut d = png::Decoder::new(std::io::Cursor::new(png_bytes));
    d.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16 | png::Transformations::ALPHA);
    let mut r = d.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    if w == 0 || h == 0 {
        return None;
    }
    let channels = info.color_type.samples();
    let px = |x: usize, y: usize| -> [u8; 4] {
        let i = (y * w + x) * channels;
        match channels {
            4 => [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]],
            3 => [buf[i], buf[i + 1], buf[i + 2], 255],
            2 => [buf[i], buf[i], buf[i], buf[i + 1]],
            _ => [buf[i], buf[i], buf[i], 255],
        }
    };
    // Box filter down (or nearest up) to 128.
    let mut out = vec![0u8; SIZE * SIZE * 4];
    for oy in 0..SIZE {
        for ox in 0..SIZE {
            let (sx0, sx1) = (ox * w / SIZE, ((ox + 1) * w / SIZE).max(ox * w / SIZE + 1));
            let (sy0, sy1) = (oy * h / SIZE, ((oy + 1) * h / SIZE).max(oy * h / SIZE + 1));
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            for y in sy0..sy1.min(h) {
                for x in sx0..sx1.min(w) {
                    let p = px(x, y);
                    let a = u32::from(p[3]);
                    acc[0] += u32::from(p[0]) * a;
                    acc[1] += u32::from(p[1]) * a;
                    acc[2] += u32::from(p[2]) * a;
                    acc[3] += a;
                    n += 1;
                }
            }
            let o = (oy * SIZE + ox) * 4;
            if acc[3] > 0 {
                out[o] = (acc[0] / acc[3]) as u8;
                out[o + 1] = (acc[1] / acc[3]) as u8;
                out[o + 2] = (acc[2] / acc[3]) as u8;
            }
            out[o + 3] = (acc[3] / n.max(1)) as u8;
        }
    }
    Some(out)
}

fn sprite(i: usize) -> String {
    format!("emotecraft:icon/slot{i}")
}

/// Draws icon `key` (its PNG `bytes`) over `[x, y, w, h]`; if it isn't in the atlas yet, asks for
/// it (it shows from the next frame).
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, key: u128, bytes: &Arc<Vec<u8>>, rect: [f32; 4], color: u32) -> bool {
    let atlas = &game.gui.atlas;
    let slot = with(|s| {
        s.clock += 1;
        let clock = s.clock;
        match s.slots.get_mut(&key) {
            Some(e) => {
                e.1 = clock;
                Some(e.0)
            }
            None => {
                if !s.wanted.iter().any(|(k, _)| *k == key) {
                    s.wanted.push((key, bytes.clone()));
                }
                None
            }
        }
    });
    let Some(slot) = slot else { return false };
    let Some(r): Option<Rect> = atlas.sprites.get(&sprite(slot)).copied() else { return false };
    batch.rect_uv(rect[0], rect[1], rect[2], rect[3], r, color);
    true
}

/// Whether the icon can show (its PNG reads).
pub(super) fn readable(key: u128) -> bool {
    with(|s| !matches!(s.pixels.get(&key), Some(None)))
}

/// Once a frame: the icons asked for, written into their slots (`map_patches`).
pub(super) fn frame(game: &mut Game<'_>) {
    let sprites = &game.gui.atlas.sprites;
    let first = sprites.get(&sprite(0)).map(|r| [r.u0, r.v0, r.u1, r.v1]);
    let Some(first) = first else { return };
    let now = (game.gui.atlas.width, game.gui.atlas.height, first);
    let wanted = with(|s| {
        if s.atlas != Some(now) {
            // A new atlas (resources reloaded): every slot is empty again.
            s.atlas = Some(now);
            s.slots.clear();
        }
        std::mem::take(&mut s.wanted)
    });
    for (key, bytes) in wanted {
        let pixels = with(|s| s.pixels.get(&key).cloned());
        let pixels = match pixels {
            Some(p) => p,
            None => {
                let p = decode(&bytes).map(Arc::new);
                with(|s| s.pixels.insert(key, p.clone()));
                p
            }
        };
        let Some(pixels) = pixels else { continue };
        let slot = with(|s| {
            let used: Vec<usize> = s.slots.values().map(|e| e.0).collect();
            let slot = (0..SLOTS).find(|i| !used.contains(i)).or_else(|| {
                let (&k, &(slot, _)) = s.slots.iter().min_by_key(|(_, e)| e.1)?;
                s.slots.remove(&k);
                Some(slot)
            })?;
            s.slots.insert(key, (slot, s.clock));
            Some(slot)
        });
        let Some(slot) = slot else { continue };
        let Some(r) = game.gui.atlas.sprites.get(&sprite(slot)).copied() else { continue };
        let (w, h) = (game.gui.atlas.width as f32, game.gui.atlas.height as f32);
        game.map_patches.push(horizon_client::font::Patch {
            x: (r.u0 * w).round() as u32,
            y: (r.v0 * h).round() as u32,
            width: SIZE as u32,
            height: SIZE as u32,
            rgba: pixels.to_vec(),
        });
    }
}
