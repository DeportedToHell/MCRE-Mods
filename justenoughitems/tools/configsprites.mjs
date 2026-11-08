// Makes mixins/justenoughitems/config_sprites.rs from MezzConfig GUI's
// textures, which JEI's config screen (MezzConfig GUI) draws, and JEI's icon
// for the screen's mod tab:
//   node tools/configsprites.mjs <MezzConfigGui checkout> <JEI checkout> [out.rs]
// MezzConfig GUI (https://github.com/mezz/MezzConfigGui) and JEI are MIT.
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { join } from "node:path";

const [gui, jei, out = new URL("../mixins/justenoughitems/config_sprites.rs", import.meta.url)] = process.argv.slice(2);
const dir = join(gui, "Common/src/main/resources/assets/mezz_config/textures/mezz_config/atlas/gui");

const SPRITES = [
  ["GUI_BACKGROUND", "gui_background"],
  ["BUTTON_DISABLED", "button_disabled"],
  ["BUTTON_ENABLED", "button_enabled"],
  ["BUTTON_HIGHLIGHT", "button_highlight"],
  ["SEARCH_BACKGROUND", "search_background"],
  ["SCROLLBAR_BACKGROUND", "scrollbar_background"],
  ["SCROLLBAR_MARKER", "scrollbar_marker"],
  ["MOD_TAB_SELECTED", "mod_tab_selected"],
  ["CHECKBOX", "checkbox"],
  ["CHECKBOX_HIGHLIGHT", "checkbox_highlight"],
  ["ICON_CHECK", "icons/check"],
  ["ICON_X", "icons/x"],
  ["ICON_RESET", "icons/reset"],
  ["ICON_ARROW_DOWN", "icons/arrow_down"],
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

function mcmeta(file) {
  if (!existsSync(file)) return null;
  const meta = JSON.parse(readFileSync(file, "utf8"));
  const s = meta?.gui?.scaling;
  if (!s || s.type !== "nine_slice") return null;
  const b = s.border;
  const [left, top, right, bottom] = typeof b === "number" ? [b, b, b, b] : [b.left, b.top, b.right, b.bottom];
  return { left, top, right, bottom };
}

let rs = `// MezzConfig GUI's textures (MIT, https://github.com/mezz/MezzConfigGui,
// Common/src/main/resources/assets/mezz_config/textures/mezz_config/atlas/gui),
// which JEI's config screen draws, and JEI's icon (MIT) at the 24x24 its mod
// tab draws it. Every pixel, row by row, as the game's rgba() packs it. Made by
// tools/configsprites.mjs; don't edit by hand.

pub(super) struct Sprite {
    pub(super) width: u16,
    pub(super) height: u16,
    /// Nine-slice borders (left, top, right, bottom): the middle is tiled.
    pub(super) border: Option<[u16; 4]>,
    pub(super) pixels: &'static [u32],
}
`;
const pack = (rgba) => rgba.map(([r, g, b, a]) => ((a << 24) | (b << 16) | (g << 8) | r) >>> 0);
const emit = (name, desc, width, height, border, packed) => {
  const pixels = packed.map((c) => `0x${c.toString(16).padStart(8, "0")}`);
  rs += `\n/// ${desc}.
pub(super) const ${name}: Sprite = Sprite {
    width: ${width},
    height: ${height},
    border: ${border ? `Some([${border.left}, ${border.top}, ${border.right}, ${border.bottom}])` : "None"},
    pixels: &[${pixels.join(",")}],
};
`;
};
for (const [name, file] of SPRITES) {
  const png = join(dir, `${file}.png`);
  const { width, height, rgba } = decode(png);
  const border = mcmeta(`${png}.mcmeta`);
  emit(name, `${file}.png, ${width}x${height}${border ? `, nine-slice ${border.left} ${border.top} ${border.right} ${border.bottom}` : ""}`, width, height, border, pack(rgba));
  console.log(`${name}: ${width}x${height}${border ? ` border ${JSON.stringify(border)}` : ""}`);
}
// JEI's icon (its fabric.mod.json's jei-icon.png), scaled to 24x24 as the
// screen's mod tab draws it (nearest texel).
{
  const { width, height, rgba } = decode(join(jei, "Common/src/main/resources/jei-icon.png"));
  const size = 24;
  const scaled = [];
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const sx = Math.min(width - 1, Math.floor(((x + 0.5) * width) / size));
      const sy = Math.min(height - 1, Math.floor(((y + 0.5) * height) / size));
      scaled.push(rgba[sy * width + sx]);
    }
  }
  emit("JEI_ICON", `jei-icon.png (${width}x${height}) at 24x24`, size, size, null, pack(scaled));
  console.log(`JEI_ICON: ${width}x${height} -> 24x24`);
}
writeFileSync(out, rs);
