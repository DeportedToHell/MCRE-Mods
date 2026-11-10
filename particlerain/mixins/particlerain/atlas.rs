// Particle Rain's sprites in the game's atlas (`SpriteLoaderMixin`): while the atlas is packed,
// the rain and snow textures the resources have (`environment/rain`, `environment/snow`) are
// split into 16 sprites each (`rain_0..15`, `snow_0..15`), four of each merged into the heavy
// rain and snow sprites (`heavy_rain_0..3`, `heavy_snow_0..3`), eight ripple rings are drawn
// (`ripple_0..7`), grey copies of the splash particle made to take the water's colour
// (`splash_0..3`), and the original's own textures added (`fog_dithered`, `fog_translucent`,
// `horizontal_fog`, `new_custom_particle`, `streak`), all as `particlerain:particle/<name>`.
// The sprites are packed like every other texture, so they need no room of the mod-image slot.
// Spliced into horizon-client-render's hud.rs as `particlerain_atlas`.
//
// Particle Rain by PigCart (https://github.com/PigCart/particle-rain), MIT.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// `compat.waterTint`: the rain is made grey for the tint, and the grey splashes exist.
pub static WATER_TINT: AtomicBool = AtomicBool::new(true);
/// `compat.rippleUsePackRes`: the ripples take the campfire smoke's resolution.
pub static RIPPLE_PACK_RES: AtomicBool = AtomicBool::new(true);
/// `compat.rippleRes`.
pub static RIPPLE_RES: AtomicU32 = AtomicU32::new(16);
/// What the last atlas was packed with (`water tint | ripple settings << 1`), 0 before the first:
/// the game reloads the resources when the settings change it.
pub static BUILT: AtomicU32 = AtomicU32::new(0);

/// The settings the sprites depend on, as `BUILT` keeps them.
pub fn wanted() -> u32 {
    let res = RIPPLE_RES.load(Ordering::Relaxed).min(0xFFFF);
    1 << 31 | u32::from(WATER_TINT.load(Ordering::Relaxed)) | u32::from(RIPPLE_PACK_RES.load(Ordering::Relaxed)) << 1 | res << 2
}

/// The atlas name of a sprite id (`minecraft:generic_0` → `particle/generic_0`,
/// `particlerain:rain_0` → `particlerain:particle/rain_0`).
pub fn sprite_name(id: &str) -> String {
    match id.split_once(':') {
        Some(("minecraft", path)) => format!("particle/{path}"),
        Some((ns, path)) => format!("{ns}:particle/{path}"),
        None => format!("particle/{id}"),
    }
}

type Image = (String, u32, u32, Vec<u8>);

struct Pixels {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

impl Pixels {
    fn new(w: u32, h: u32) -> Self {
        Pixels { w, h, rgba: vec![0; (w * h * 4) as usize] }
    }
    fn get(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.w || y >= self.h {
            return [0; 4];
        }
        let i = ((y * self.w + x) * 4) as usize;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }
    fn set(&mut self, x: u32, y: u32, p: [u8; 4]) {
        if x >= self.w || y >= self.h {
            return;
        }
        let i = ((y * self.w + x) * 4) as usize;
        self.rgba[i..i + 4].copy_from_slice(&p);
    }
    /// `TextureUtil.boostAlpha`: an image whose most opaque pixel is not opaque has its alpha
    /// multiplied by `255 / highest` (whole numbers).
    fn boost_alpha(&mut self) {
        let highest = self.rgba.chunks_exact(4).map(|p| u32::from(p[3])).max().unwrap_or(0);
        if highest == 255 || highest == 0 {
            return;
        }
        let multiplier = 255 / highest;
        if multiplier == 1 {
            return;
        }
        for p in self.rgba.chunks_exact_mut(4) {
            p[3] = ((u32::from(p[3]) * multiplier) & 0xFF) as u8;
        }
    }
    /// `TextureUtil.desaturate`: grey at the brightest channel.
    fn desaturate(&mut self) {
        for p in self.rgba.chunks_exact_mut(4) {
            let g = p[0].max(p[1]).max(p[2]);
            p[0] = g;
            p[1] = g;
            p[2] = g;
        }
    }
    fn image(self, name: String) -> Image {
        (name, self.w, self.h, self.rgba)
    }
}

/// `TextureUtil.getSpriteFromSheet`: sprite `i` of a sheet `columns` wide, read down each column,
/// turned half round (`copyRect` with both flips).
fn sprite_from_sheet(sheet: &Pixels, i: u32, columns: u32) -> Option<Pixels> {
    let size = sheet.w / columns;
    if size == 0 {
        return None;
    }
    let rows = sheet.h / size;
    if rows == 0 || i >= rows * columns {
        return None;
    }
    let (row, col) = (i % rows, i / rows);
    let mut out = Pixels::new(size, size);
    for y in 0..size {
        for x in 0..size {
            out.set(size - 1 - x, size - 1 - y, sheet.get(size * col + x, size * row + y));
        }
    }
    Some(out)
}

/// `TextureUtil.mergeSpritesFromSheet`: sprites `start..start + n` laid over each other, a pixel
/// kept where its alpha is above the sheet's own pixel at the target's place
/// (`copyRectHighestAlpha` reads the sheet there, not the target).
fn merge_from_sheet(sheet: &Pixels, start: u32, n: u32, columns: u32) -> Option<Pixels> {
    let size = sheet.w / columns;
    if size == 0 {
        return None;
    }
    let rows = sheet.h / size;
    if start + n > rows * columns {
        return None;
    }
    let mut out = Pixels::new(size, size);
    for i in start..start + n {
        let (row, col) = (i % rows, i / rows);
        for y in 0..size {
            for x in 0..size {
                let (dx, dy) = (size - 1 - x, size - 1 - y);
                let src = sheet.get(size * col + x, size * row + y);
                let trg = sheet.get(dx, dy);
                if src[3] > trg[3] {
                    out.set(dx, dy, src);
                }
            }
        }
    }
    Some(out)
}

/// `TextureUtil.generateRipple`: a white ring of radius `size / 16 * (i + 1)`.
fn ripple(i: u32, size: u32) -> Pixels {
    let mut img = Pixels::new(size, size);
    let radius = ((size as f32 / 2.0) / 8.0) * (i + 1) as f32;
    let r = radius.min(size as f32 / 2.0 - 1.0).max(1.0) as i32;
    let c = (size / 2) as i32;
    let white = [255; 4];
    let mut plot = |x: i32, y: i32| {
        for (px, py) in [(c + x, c + y), (c - x, c + y), (c + x, c - y), (c - x, c - y), (c + y, c + x), (c - y, c + x), (c + y, c - x), (c - y, c - x)] {
            if px >= 0 && py >= 0 {
                img.set(px as u32, py as u32, white);
            }
        }
    };
    let (mut x, mut y) = (0i32, r);
    let mut d = 3 - 2 * r;
    plot(x, y);
    while y >= x {
        if d > 0 {
            y -= 1;
            d += 4 * (x - y) + 10;
        } else {
            d += 4 * x + 6;
        }
        x += 1;
        plot(x, y);
    }
    img
}

fn find(images: &[Image], name: &str) -> Option<Pixels> {
    images.iter().find(|i| i.0 == name).map(|i| Pixels { w: i.1, h: i.2, rgba: i.3.clone() })
}

/// The mod's sprites into the images the atlas packs (before it lays them out). A sprite a
/// resource pack already gives (`assets/particlerain/textures/particle/<name>.png`) stays the pack's.
pub fn add_images(images: &mut Vec<Image>) {
    let water_tint = WATER_TINT.load(Ordering::Relaxed);
    let mut new: Vec<Image> = Vec::new();
    let add = |name: String, p: Option<Pixels>, new: &mut Vec<Image>| {
        if let Some(p) = p {
            new.push(p.image(name));
        }
    };
    let rain = find(images, "environment/rain").map(|mut p| {
        p.boost_alpha();
        if water_tint {
            p.desaturate();
        }
        p
    });
    let snow = find(images, "environment/snow").map(|mut p| {
        p.boost_alpha();
        p
    });
    for (prefix, sheet) in [("rain", &rain), ("snow", &snow)] {
        let Some(sheet) = sheet else {
            log::error!("Particle Rain: error loading the {prefix} texture");
            continue;
        };
        for i in 0..16 {
            add(format!("particlerain:particle/{prefix}_{i}"), sprite_from_sheet(sheet, i, 2), &mut new);
        }
        for i in 0..4 {
            add(format!("particlerain:particle/heavy_{prefix}_{i}"), merge_from_sheet(sheet, i * 4, 4, 2), &mut new);
        }
    }
    // `getRippleResolution`: the campfire smoke's width (at most 256), else the setting (4..=256).
    let pack_res = RIPPLE_PACK_RES.load(Ordering::Relaxed);
    let res = images
        .iter()
        .find(|i| i.0 == "particle/big_smoke_0")
        .filter(|_| pack_res)
        .map(|i| i.1.min(256))
        .unwrap_or_else(|| RIPPLE_RES.load(Ordering::Relaxed).clamp(4, 256));
    for i in 0..8 {
        add(format!("particlerain:particle/ripple_{i}"), Some(ripple(i, res)), &mut new);
    }
    if water_tint {
        for i in 0..4 {
            let splash = find(images, &format!("particle/splash_{i}")).map(|mut p| {
                p.desaturate();
                p
            });
            add(format!("particlerain:particle/splash_{i}"), splash, &mut new);
        }
    }
    for (name, png) in super::particlerain_textures::TEXTURES {
        match super::decode_bytes(png) {
            Ok((w, h, rgba)) => new.push((format!("particlerain:particle/{name}"), w, h, rgba)),
            Err(e) => log::error!("Particle Rain: error loading {name}: {e}"),
        }
    }
    for image in new {
        if !images.iter().any(|i| i.0 == image.0) {
            images.push(image);
        }
    }
    BUILT.store(wanted(), Ordering::Relaxed);
}
