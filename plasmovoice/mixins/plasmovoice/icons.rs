// Plasmo Voice's icons (`assets/plasmovoice/textures/icons`, its own PNGs in data/), and the
// ones a server sends as `base64;<png>` (`VoiceIconUtil`), decoded once into pixel runs and
// drawn with the game's fills: no texture of the mod's own in the atlas.
//
// This Source Code Form is a port of Plasmo Voice (https://github.com/plasmoapp/plasmo-voice), LGPL-3.0.

use horizon_client::hud::{HudAtlas, HudBatch, rgba};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// An icon's pixels as runs of one colour: x, y, length, ARGB.
pub(super) struct Icon {
    pub w: u32,
    pub h: u32,
    runs: Vec<(u16, u16, u16, u32)>,
}

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        const FILES: &[(&str, &[u8])] = &[$(($name, horizon_client::native_data!("plasmovoice", $name))),*];
    };
}

icons!(
    "eye.png", "eye_disabled.png", "headset_disabled.png", "headset_not_installed.png", "microphone.png", "microphone_amogus.png",
    "microphone_broadcast.png", "microphone_disabled.png", "microphone_disconnected.png", "microphone_group.png", "microphone_menu.png",
    "microphone_menu_disabled.png", "microphone_muted.png", "microphone_priority.png", "microphone_radio.png", "microphone_whisper.png",
    "reset.png", "speaker.png", "speaker_broadcast.png", "speaker_disabled.png", "speaker_disc.png", "speaker_group.png", "speaker_menu.png",
    "speaker_menu_disabled.png", "speaker_muted.png", "speaker_priority.png", "speaker_radio.png", "speaker_whisper.png", "warning.png",
    "tab_activation.png", "tab_activation_disabled.png", "tab_addons.png", "tab_addons_disabled.png", "tab_advanced.png",
    "tab_advanced_disabled.png", "tab_devices.png", "tab_devices_disabled.png", "tab_hotkeys.png", "tab_hotkeys_disabled.png",
    "tab_overlay.png", "tab_overlay_disabled.png", "tab_plasmo.png", "tab_volume.png", "tab_volume_disabled.png",
);

/// RGBA pixels of a PNG.
fn decode(bytes: &[u8]) -> Option<(u32, u32, Vec<[u8; 4]>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let px: Vec<[u8; 4]> = match info.color_type {
        png::ColorType::Rgba => buf.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect(),
        png::ColorType::Rgb => buf.chunks_exact(3).map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks_exact(2).map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => buf.iter().map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some((info.width, info.height, px))
}

fn icon(bytes: &[u8]) -> Option<Icon> {
    let (w, h, px) = decode(bytes)?;
    if w == 0 || h == 0 || w > 256 || h > 256 {
        return None;
    }
    let mut runs = Vec::new();
    for y in 0..h {
        let mut x = 0;
        while x < w {
            let [r, g, b, a] = px[(y * w + x) as usize];
            let mut len = 1;
            while x + len < w && px[(y * w + x + len) as usize] == [r, g, b, a] {
                len += 1;
            }
            if a > 0 {
                runs.push((x as u16, y as u16, len as u16, u32::from_be_bytes([a, r, g, b])));
            }
            x += len;
        }
    }
    Some(Icon { w, h, runs })
}

fn builtin() -> &'static HashMap<&'static str, Icon> {
    static ICONS: OnceLock<HashMap<&'static str, Icon>> = OnceLock::new();
    ICONS.get_or_init(|| {
        FILES.iter().filter_map(|(file, bytes)| Some((file.strip_suffix(".png").unwrap_or(file), icon(bytes)?))).collect()
    })
}

/// Standard base64 (`Base64.getDecoder`).
fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// A server's `base64;` icons, decoded once (`registerBase64Texture`).
fn sent(icon_str: &str, f: impl FnOnce(Option<&Icon>)) {
    static SENT: Mutex<Option<HashMap<String, Option<Icon>>>> = Mutex::new(None);
    let mut guard = SENT.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    let entry = map.entry(icon_str.to_owned()).or_insert_with(|| {
        let data = icon_str.strip_prefix("base64;")?;
        icon(&base64(data)?)
    });
    f(entry.as_ref());
}

/// The icon for a resource location (`plasmovoice:textures/icons/microphone.png`), a
/// `base64;` icon, or a short name (`microphone`, `tab_devices`).
pub(super) fn with<R>(location: &str, f: impl FnOnce(Option<&Icon>) -> R) -> R {
    if location.starts_with("base64;") {
        let mut out = None;
        sent(location, |i| out = Some(f(i)));
        return out.expect("ran");
    }
    let name = location
        .strip_prefix("plasmovoice:textures/icons/")
        .map(|n| n.strip_suffix(".png").unwrap_or(n).replace("tabs/", "tab_"))
        .unwrap_or_else(|| location.to_owned());
    f(builtin().get(name.as_str()))
}

/// The ARGB product of two colours (`setShaderColor` on a texture).
fn tint(px: u32, color: u32) -> u32 {
    let a = px.to_be_bytes();
    let b = color.to_be_bytes();
    let m = |i: usize| ((u32::from(a[i]) * u32::from(b[i]) + 127) / 255) as u8;
    let [aa, r, g, bb] = [m(0), m(1), m(2), m(3)];
    rgba(r, g, bb, aa)
}

/// `blit(icon, x, y, 0, 0, w, h, w, h)` tinted with ARGB `color`.
pub(super) fn draw(batch: &mut HudBatch, atlas: &HudAtlas, location: &str, x: f32, y: f32, w: f32, h: f32, color: u32) {
    with(location, |icon| {
        let Some(icon) = icon else { return };
        let (sx, sy) = (w / icon.w as f32, h / icon.h as f32);
        for &(px, py, len, c) in &icon.runs {
            batch.fill(atlas, x + f32::from(px) * sx, y + f32::from(py) * sy, f32::from(len) * sx, sy, tint(c, color));
        }
    });
}

/// `blitIcon`: the icon's silhouette one pixel down and right in `shadow` × 0.25 (the
/// `TEXTURE_SOLID_COLOR` pass), then the icon in `color`.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_shadowed(batch: &mut HudBatch, atlas: &HudAtlas, location: &str, x: f32, y: f32, size: f32, color: u32, shadow: Option<u32>) {
    if let Some(shadow) = shadow {
        let [a, r, g, b] = shadow.to_be_bytes();
        let quarter = |c: u8| (f64::from(c) * 0.25) as u8;
        let solid = u32::from_be_bytes([a, quarter(r), quarter(g), quarter(b)]);
        with(location, |icon| {
            let Some(icon) = icon else { return };
            let (sx, sy) = (size / icon.w as f32, size / icon.h as f32);
            for &(px, py, len, c) in &icon.runs {
                let alpha = (c >> 24) as u8;
                let [sa, sr, sg, sb] = solid.to_be_bytes();
                let a = ((u32::from(alpha) * u32::from(sa)) / 255) as u8;
                batch.fill(atlas, x + 1.0 + f32::from(px) * sx, y + 1.0 + f32::from(py) * sy, f32::from(len) * sx, sy, rgba(sr, sg, sb, a));
            }
        });
    }
    draw(batch, atlas, location, x, y, size, size, color);
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_icon_decodes() {
        for (file, _) in super::FILES {
            assert!(super::builtin().contains_key(file.strip_suffix(".png").unwrap()), "{file}");
        }
        assert_eq!(super::base64("aGk=").unwrap(), b"hi");
    }
}
