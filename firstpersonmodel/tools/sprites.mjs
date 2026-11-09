// Makes mixins/firstpersonmodel/sprites.rs from TRender's textures:
//   node tools/sprites.mjs <TRender checkout> [out.rs]
// TRender (https://github.com/tr7zw/TRender, MIT, by tr7zw) is the GUI library
// First-person Model builds its config screen with. The mod draws these
// textures pixel by pixel with the game's fills (nine-sliced as the game
// nine-slices its sprites), so it needs no texture of its own in the atlas.
import { readFileSync, writeFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { join } from "node:path";

const [checkout, out = new URL("../mixins/firstpersonmodel/sprites.rs", import.meta.url)] = process.argv.slice(2);
const dir = join(checkout, "src/main/resources/assets/trender/textures");

// Name, file, and the square size TRender squeezes it into (its 20x20
// checkboxes are drawn 18x18).
const SPRITES = [
  ["BUTTON", "gui/sprites/widget/button_dark.png"],
  ["BUTTON_HIGHLIGHTED", "gui/sprites/widget/button_highlighted_dark.png"],
  ["SLIDER_HANDLE", "gui/sprites/widget/slider_handle_dark.png"],
  ["SLIDER_HANDLE_HIGHLIGHTED", "gui/sprites/widget/slider_handle_highlighted_dark.png"],
  ["CHECKBOX", "widget/checkbox.png", 18],
  ["CHECKBOX_SELECTED", "widget/checkbox_selected.png", 18],
  ["CHECKBOX_HIGHLIGHTED", "widget/checkbox_highlighted.png", 18],
];

function decode(file) {
  const buf = readFileSync(file);
  let pos = 8;
  let width = 0, height = 0, depth = 0, type = 0;
  const idat = [];
  let palette = null, trns = null;
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos);
    const kind = buf.toString("ascii", pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (kind === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      depth = data[8];
      type = data[9];
      if (data[12] !== 0) throw new Error(`${file}: interlaced`);
    } else if (kind === "PLTE") palette = data;
    else if (kind === "tRNS") trns = data;
    else if (kind === "IDAT") idat.push(data);
    pos += 12 + len;
  }
  const channels = { 0: 1, 2: 3, 3: 1, 4: 2, 6: 4 }[type];
  const raw = inflateSync(Buffer.concat(idat));
  const bits = channels * depth;
  const bpp = Math.max(1, bits >> 3);
  const stride = Math.ceil((width * bits) / 8);
  const bytes = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? bytes[y * stride + x - bpp] : 0;
      const b = y > 0 ? bytes[(y - 1) * stride + x] : 0;
      const c = x >= bpp && y > 0 ? bytes[(y - 1) * stride + x - bpp] : 0;
      let v = line[x];
      if (filter === 1) v += a;
      else if (filter === 2) v += b;
      else if (filter === 3) v += (a + b) >> 1;
      else if (filter === 4) {
        const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      bytes[y * stride + x] = v & 255;
    }
  }
  // One sample of channel ch of pixel x on row y, scaled to 0..255.
  const sample = (x, y, ch) => {
    const index = x * channels + ch;
    if (depth === 8) return bytes[y * stride + index];
    if (depth === 16) return bytes[y * stride + index * 2];
    const bit = index * depth;
    const byte = bytes[y * stride + (bit >> 3)];
    const v = (byte >> (8 - depth - (bit & 7))) & ((1 << depth) - 1);
    return type === 3 ? v : Math.round((v * 255) / ((1 << depth) - 1));
  };
  const rgba = [];
  for (let i = 0; i < width * height; i++) {
    const p = Array.from({ length: channels }, (_, ch) => sample(i % width, Math.floor(i / width), ch));
    let r, g, b, a = 255;
    if (type === 6) [r, g, b, a] = p;
    else if (type === 2) [r, g, b] = p;
    else if (type === 4) [r, a] = p, g = b = r;
    else if (type === 0) r = g = b = p[0];
    else if (type === 3) {
      [r, g, b] = [palette[p[0] * 3], palette[p[0] * 3 + 1], palette[p[0] * 3 + 2]];
      a = trns && p[0] < trns.length ? trns[p[0]] : 255;
    }
    rgba.push([r, g, b, a]);
  }
  return { width, height, rgba };
}


let rs = `// TRender's textures (MIT, https://github.com/tr7zw/TRender,
// src/main/resources/assets/trender/textures), pixel by pixel, each colour as
// the game's rgba() packs it (0 = transparent). Made by tools/sprites.mjs;
// don't edit by hand.

pub(super) struct Sprite {
    pub(super) w: u16,
    pub(super) h: u16,
    pub(super) px: &'static [u32],
}
`;
for (const [name, file, squeeze] of SPRITES) {
  const png = decode(join(dir, file));
  let w = png.width;
  let h = png.height;
  let at = (x, y) => png.rgba[y * png.width + x];
  if (squeeze) {
    // The whole image blitted into squeeze x squeeze, nearest-sampled.
    const src = at;
    const [sw, sh] = [w, h];
    at = (x, y) => src(Math.floor(((x + 0.5) * sw) / squeeze), Math.floor(((y + 0.5) * sh) / squeeze));
    w = h = squeeze;
  }
  const rows = [];
  for (let y = 0; y < h; y++) {
    const row = [];
    for (let x = 0; x < w; x++) {
      const [r, g, b, a] = at(x, y);
      row.push(a === 0 ? "0" : "0x" + (((a << 24) | (b << 16) | (g << 8) | r) >>> 0).toString(16).padStart(8, "0"));
    }
    rows.push("        " + row.join(", ") + ",");
  }
  const note = squeeze ? ` (squeezed from ${png.width}x${png.height})` : "";
  rs += `
/// ${file}, ${w}x${h}${note}.
#[rustfmt::skip]
pub(super) const ${name}: Sprite = Sprite {
    w: ${w},
    h: ${h},
    px: &[
${rows.join("\n")}
    ],
};
`;
  console.log(`${name}: ${w}x${h}`);
}
writeFileSync(out, rs);
