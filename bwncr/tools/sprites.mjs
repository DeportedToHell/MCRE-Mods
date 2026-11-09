// Makes mixins/bwncr/cloth_sprites.rs from Cloth Config's texture:
//   node tools/sprites.mjs <Cloth Config resources: the folder with assets/> [out.rs]
// (a Cloth Config checkout's common/src/main/resources, or its jar unpacked).
// cloth_config.png (assets/cloth-config2/textures/gui) is Cloth Config's,
// LGPL-3.0 as the rest of Cloth Config. The screen draws it pixel by pixel
// with the game's fills, so it needs no texture of its own in the atlas.
// After our ClientSort port's tools/sprites.mjs.
import { readFileSync, writeFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { join } from "node:path";

const [checkout, out = new URL("../mixins/bwncr/cloth_sprites.rs", import.meta.url)] = process.argv.slice(2);
if (!checkout) throw new Error("usage: node tools/sprites.mjs <Cloth Config resources> [out.rs]");
const file = join(checkout, "assets/cloth-config2/textures/gui/cloth_config.png");

// The parts the screen draws: name, region (u, v, w, h).
const SPRITES = [
  // A sub-category's arrow (`SubCategoryListEntry`): closed and open, plain and hovered.
  ["CATEGORY", [24, 0, 9, 9]],
  ["CATEGORY_OPEN", [24, 9, 9, 9]],
  ["CATEGORY_HOVER", [24, 18, 9, 9]],
  ["CATEGORY_OPEN_HOVER", [24, 27, 9, 9]],
  // A list's arrow (`BaseListEntry`): closed and open, plain and hovered.
  ["EXPAND", [33, 0, 9, 9]],
  ["EXPAND_OPEN", [33, 9, 9, 9]],
  ["EXPAND_HOVER", [33, 18, 9, 9]],
  ["EXPAND_OPEN_HOVER", [33, 27, 9, 9]],
  // Insert New, plain and hovered; Delete Selected, off, plain, hovered.
  ["INSERT", [42, 0, 9, 9]],
  ["INSERT_HOVER", [42, 9, 9, 9]],
  ["DELETE_OFF", [51, 0, 9, 9]],
  ["DELETE", [51, 9, 9, 9]],
  ["DELETE_HOVER", [51, 18, 9, 9]],
];

function decode(path) {
  const buf = readFileSync(path);
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
      if (data[12] !== 0) throw new Error(`${path}: interlaced`);
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

const png = decode(file);
let rs = `// Cloth Config's texture (LGPL-3.0, https://github.com/shedaniel/cloth-config,
// assets/cloth-config2/textures/gui/cloth_config.png), as runs
// of pixels: (x, y, length, colour as the game's rgba() packs it). Made by
// tools/sprites.mjs; don't edit by hand.

pub(super) struct Sprite {
    pub(super) runs: &'static [(u8, u8, u8, u32)],
}
`;
for (const [name, region] of SPRITES) {
  const [u, v, width, height] = region;
  const at = (x, y) => png.rgba[(v + y) * png.width + u + x];
  const runs = [];
  for (let y = 0; y < height; y++) {
    let x = 0;
    while (x < width) {
      const [r, g, b, a] = at(x, y);
      const c = ((a << 24) | (b << 16) | (g << 8) | r) >>> 0;
      if (a === 0) { x++; continue; }
      let len = 1;
      while (x + len < width && len < 255) {
        const [r2, g2, b2, a2] = at(x + len, y);
        if ((((a2 << 24) | (b2 << 16) | (g2 << 8) | r2) >>> 0) !== c) break;
        len++;
      }
      runs.push(`(${x},${y},${len},0x${c.toString(16).padStart(8, "0")})`);
      x += len;
    }
  }
  rs += `\n/// cloth_config.png (${region.join(", ")}), ${width}x${height}.
pub(super) const ${name}: Sprite = Sprite {
    runs: &[${runs.join(",")}],
};
`;
  console.log(`${name}: ${width}x${height}, ${runs.length} runs`);
}
writeFileSync(out, rs);
