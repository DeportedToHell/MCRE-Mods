// Makes mixins/shulkerboxtooltip/sprites.rs from ShulkerBoxTooltip's preview
// window texture: `node tools/sprites.mjs <ShulkerBoxTooltip checkout> [out.rs]`.
// shulker_box_tooltip.png (common/src/main/resources/assets/shulkerboxtooltip/
// textures/gui/sprites) is ShulkerBoxTooltip's own (MIT). The game draws it
// pixel by pixel with its fills, tinted with the window's colour, so it needs
// no texture of its own in the atlas.
import { readFileSync, writeFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { join } from "node:path";

const [checkout, out = new URL("../mixins/shulkerboxtooltip/sprites.rs", import.meta.url)] = process.argv.slice(2);
const file = join(checkout, "common/src/main/resources/assets/shulkerboxtooltip/textures/gui/sprites/shulker_box_tooltip.png");
const meta = JSON.parse(readFileSync(file + ".mcmeta", "utf8")).gui.scaling;

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
if (png.width !== meta.width || png.height !== meta.height) throw new Error("the texture is not its gui.scaling size");
// Runs of equal pixels on each row: (x, y, length, [r, g, b, a]).
const runs = [];
for (let y = 0; y < png.height; y++) {
  let x = 0;
  while (x < png.width) {
    const c = png.rgba[y * png.width + x];
    if (c[3] === 0) { x++; continue; }
    let len = 1;
    while (x + len < png.width && png.rgba[y * png.width + x + len].every((v, i) => v === c[i])) len++;
    runs.push(`(${x},${y},${len},[${c.join(",")}])`);
    x += len;
  }
}
const rs = `// ShulkerBoxTooltip's preview window (MIT, https://github.com/MisterPeModder/ShulkerBoxTooltip,
// common/src/main/resources/assets/shulkerboxtooltip/textures/gui/sprites/shulker_box_tooltip.png),
// as runs of pixels: (x, y, length, [r, g, b, a]). Made by tools/sprites.mjs;
// don't edit by hand.

/// The texture's size, and its nine-slice border (\`gui.scaling\` of its .mcmeta;
/// the edges and the middle are tiled, \`stretch_inner\` false).
pub(super) const WINDOW_SIZE: u8 = ${png.width};
pub(super) const WINDOW_BORDER: u8 = ${meta.border};

pub(super) const WINDOW: &[(u8, u8, u8, [u8; 4])] = &[${runs.join(",")}];
`;
writeFileSync(out, rs);
console.log(`${png.width}x${png.height}, border ${meta.border}, ${runs.length} runs`);
