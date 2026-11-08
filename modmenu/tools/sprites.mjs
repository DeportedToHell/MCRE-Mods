// Makes mixins/modmenu/sprites.rs from Mod Menu's textures:
//   node tools/sprites.mjs <ModMenu checkout> [out.rs]
// Mod Menu's textures (src/main/resources/assets/modmenu) are MIT, as the rest
// of Mod Menu. The mod draws them pixel by pixel with the game's fills, so it
// needs no texture of its own in the game's atlas.
import { readFileSync, writeFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { join } from "node:path";

const [checkout, out = new URL("../mixins/modmenu/sprites.rs", import.meta.url)] = process.argv.slice(2);
const dir = join(checkout, "src/main/resources/assets/modmenu");

// The parts the mod draws: name, file, region (u, v, w, h) or the whole file.
const SPRITES = [
  ["ICON", "icon.png"],
  ["UNKNOWN", "unknown_icon.png"],
  ["MINECRAFT", "minecraft_icon.png"],
  // The Mods icon itself is in title.rs: the title draws it, before the game is up too.
  ["FILTERS", "textures/gui/sprites/filters/enabled.png"],
  ["CONFIGURE", "textures/gui/sprites/configure/enabled.png"],
  // Quick configure over a hovered row's icon: white, then blue under the pointer.
  ["QUICK_CONFIGURE", "textures/gui/mod_configuration.png", [0, 0, 32, 32]],
  ["QUICK_CONFIGURE_HOVER", "textures/gui/mod_configuration.png", [0, 32, 32, 32]],
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

let rs = `// Mod Menu's textures (MIT, https://github.com/TerraformersMC/ModMenu,
// src/main/resources/assets/modmenu), as runs
// of pixels: (x, y, length, colour as the game's rgba() packs it). Made by
// tools/sprites.mjs; don't edit by hand.

pub(super) struct Sprite {
    pub(super) width: u16,
    pub(super) runs: &'static [(u8, u8, u8, u32)],
}
`;
for (const [name, file, region, nine, tiled] of SPRITES) {
  const png = decode(join(dir, file));
  let [u, v, width, height] = region ?? [0, 0, png.width, png.height];
  let at = (x, y) => png.rgba[(v + y) * png.width + u + x];
  if (nine) {
    // Corners as they are, one middle column and row: panels are plain
    // colours between their borders.
    const [w0, h0] = [width, height];
    const pick = (i, size) => (i < nine ? i : i === nine ? nine : size - (2 * nine + 1 - i));
    const src = at;
    at = (x, y) => src(pick(x, w0), pick(y, h0));
    width = height = 2 * nine + 1;
  }
  const packed = [];
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const [r, g, b, a] = at(x, y);
      packed.push(((a << 24) | (b << 16) | (g << 8) | r) >>> 0);
    }
  }
  const runs = [];
  for (let y = 0; y < height; y++) {
    let x = 0;
    while (x < width) {
      const c = packed[y * width + x];
      if (c >>> 24 === 0) { x++; continue; }
      let len = 1;
      while (x + len < width && packed[y * width + x + len] === c && len < 255) len++;
      runs.push(`(${x},${y},${len},0x${c.toString(16).padStart(8, "0")})`);
      x += len;
    }
  }

  const where = region ? ` (${region.join(", ")})` : "";
  rs += `\n/// ${file}${where}, ${width}x${height}${nine ? `, nine-slice ${nine}` : ""}.
pub(super) const ${name}: Sprite = Sprite {
    width: ${width},
    runs: &[${runs.join(",")}],
};
`;
  console.log(`${name}: ${width}x${height}, ${runs.length} runs`);
}
writeFileSync(out, rs);
